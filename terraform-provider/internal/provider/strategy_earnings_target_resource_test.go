package provider

import (
	"context"
	"io"
	"net/http"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
)

const (
	testEarningsTargetStrategyID = "00000000-0000-4000-8000-000000000201"
	testEarningsTargetRefID      = "synthetic-axis/synthetic-group"
	testEarningsTargetCreatedAt  = "2026-01-02T03:04:05Z"
)

type earningsTargetRequestObservation struct {
	Method string
	Path   string
	Query  string
	Body   string
}

func TestStrategyEarningsTargetResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 3)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		switch {
		case r.URL.Path == "/api/refs/resolve":
			writeEarningsTargetResponse(t, w, http.StatusOK, `[{"kind":"group","id":"synthetic-axis/synthetic-group","name":"Synthetic Group"}]`)
		case r.Method == http.MethodPost:
			writeEarningsTargetResponse(t, w, http.StatusOK, `{"changed":true}`)
		case r.Method == http.MethodGet:
			writeEarningsTargetResponse(t, w, http.StatusOK, `[{"ref_kind":"group","ref_id":"synthetic-axis/synthetic-group","created_at":"2026-01-02T03:04:05Z"}]`)
		default:
			writeEarningsTargetResponse(t, w, http.StatusMethodNotAllowed, `{"error":"unexpected method"}`)
		}
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyEarningsTargetModel{
		ID:         types.StringUnknown(),
		StrategyID: types.StringValue(testEarningsTargetStrategyID),
		RefKind:    types.StringValue("group"),
		RefID:      types.StringValue(testEarningsTargetRefID),
		CreatedAt:  types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema}}
	(&strategyEarningsTargetResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state strategyEarningsTargetModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	type output struct {
		Requests    []earningsTargetRequestObservation
		State       strategyEarningsTargetModel
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Requests: []earningsTargetRequestObservation{<-requests, <-requests, <-requests}, State: state, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Requests: []earningsTargetRequestObservation{
			{Method: http.MethodGet, Path: "/api/refs/resolve", Query: "link=group%3Asynthetic-axis%2Fsynthetic-group"},
			{Method: http.MethodPost, Path: "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets", Body: `{"ref_id":"synthetic-axis/synthetic-group","ref_kind":"group"}`},
			{Method: http.MethodGet, Path: "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets"},
		},
		State: strategyEarningsTargetModel{
			ID:         types.StringValue(strategyEarningsTargetID(testEarningsTargetStrategyID, "group", testEarningsTargetRefID)),
			StrategyID: types.StringValue(testEarningsTargetStrategyID),
			RefKind:    types.StringValue("group"),
			RefID:      types.StringValue(testEarningsTargetRefID),
			CreatedAt:  types.StringValue(testEarningsTargetCreatedAt),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceRejectsAliasResolvedID(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusOK, `[{"kind":"stock","id":"synthetic-canonical-id","name":"Synthetic Stock"}]`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyEarningsTargetModel{
		ID:         types.StringUnknown(),
		StrategyID: types.StringValue(testEarningsTargetStrategyID),
		RefKind:    types.StringValue("stock"),
		RefID:      types.StringValue("synthetic-alias"),
		CreatedAt:  types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema}}
	(&strategyEarningsTargetResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)

	type output struct {
		Requests    []earningsTargetRequestObservation
		StateEmpty  bool
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Requests: []earningsTargetRequestObservation{<-requests}, StateEmpty: response.State.Raw.IsNull(), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Requests:   []earningsTargetRequestObservation{{Method: http.MethodGet, Path: "/api/refs/resolve", Query: "link=stock%3Asynthetic-alias"}},
		StateEmpty: true,
		Diagnostics: []apiDiagnosticObservation{{
			Severity: "Error",
			Summary:  "Non-canonical earnings target ref_id",
			Detail:   `ref_id resolves to "synthetic-canonical-id". Specify the canonical ID to keep Terraform state aligned with the backend.`,
			Path:     "ref_id",
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("non-canonical reference output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusOK, `[{"ref_kind":"stock","ref_id":"synthetic-axis/synthetic-group","created_at":"2026-01-01T00:00:00Z"},{"ref_kind":"group","ref_id":"synthetic-axis/synthetic-group","created_at":"2026-01-02T03:04:05Z"}]`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, testEarningsTargetModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&strategyEarningsTargetResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var result strategyEarningsTargetModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)

	type output struct {
		Request     earningsTargetRequestObservation
		State       strategyEarningsTargetModel
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Request: <-requests, State: result, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request: earningsTargetRequestObservation{Method: http.MethodGet, Path: "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets"},
		State:   testEarningsTargetModel(),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceReadRemovesExternallyDeletedTarget(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusOK, `[]`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, testEarningsTargetModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&strategyEarningsTargetResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)

	type output struct {
		Request      earningsTargetRequestObservation
		StateRemoved bool
		Diagnostics  []apiDiagnosticObservation
	}
	got := output{Request: <-requests, StateRemoved: response.State.Raw.IsNull(), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request:      earningsTargetRequestObservation{Method: http.MethodGet, Path: "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets"},
		StateRemoved: true,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceReadRemovesTargetWhenStrategyIsNotFound(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusNotFound, `{"error":"synthetic missing strategy"}`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, testEarningsTargetModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&strategyEarningsTargetResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)

	type output struct {
		Request      earningsTargetRequestObservation
		StateRemoved bool
		Diagnostics  []apiDiagnosticObservation
	}
	got := output{Request: <-requests, StateRemoved: response.State.Raw.IsNull(), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request:      earningsTargetRequestObservation{Method: http.MethodGet, Path: "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets"},
		StateRemoved: true,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read not-found output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusOK, `{"changed":false}`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, testEarningsTargetModel()); diagnostics.HasError() {
		t.Fatalf("build delete state: %v", diagnostics)
	}
	response := resource.DeleteResponse{}
	(&strategyEarningsTargetResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)

	type output struct {
		Request     earningsTargetRequestObservation
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Request: <-requests, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request: earningsTargetRequestObservation{
			Method: http.MethodDelete,
			Path:   "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets",
			Query:  "ref_id=synthetic-axis%2Fsynthetic-group&ref_kind=group",
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceDeleteNotFoundIsIdempotent(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan earningsTargetRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requests <- recordEarningsTargetRequest(t, r)
		writeEarningsTargetResponse(t, w, http.StatusNotFound, `{"error":"synthetic missing strategy"}`)
	})
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, testEarningsTargetModel()); diagnostics.HasError() {
		t.Fatalf("build delete state: %v", diagnostics)
	}
	response := resource.DeleteResponse{}
	(&strategyEarningsTargetResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)

	type output struct {
		Request     earningsTargetRequestObservation
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Request: <-requests, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{Request: earningsTargetRequestObservation{
		Method: http.MethodDelete,
		Path:   "/api/strategies/" + testEarningsTargetStrategyID + "/earnings-targets",
		Query:  "ref_id=synthetic-axis%2Fsynthetic-group&ref_kind=group",
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete not-found output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceImportStatePreservesGroupIDPath(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := strategyEarningsTargetTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	initial := testEarningsTargetModel()
	initial.ID = types.StringNull()
	initial.CreatedAt = types.StringNull()
	if diagnostics := state.Set(ctx, initial); diagnostics.HasError() {
		t.Fatalf("initialize import state: %v", diagnostics)
	}
	response := resource.ImportStateResponse{State: state}
	importID := strategyEarningsTargetID(testEarningsTargetStrategyID, "group", testEarningsTargetRefID)
	(&strategyEarningsTargetResource{}).ImportState(ctx, resource.ImportStateRequest{ID: importID}, &response)
	var imported strategyEarningsTargetModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)

	got := struct {
		State       strategyEarningsTargetModel
		Diagnostics []apiDiagnosticObservation
	}{State: imported, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       strategyEarningsTargetModel
		Diagnostics []apiDiagnosticObservation
	}{
		State: strategyEarningsTargetModel{
			ID:         types.StringValue(importID),
			StrategyID: types.StringValue(testEarningsTargetStrategyID),
			RefKind:    types.StringValue("group"),
			RefID:      types.StringValue(testEarningsTargetRefID),
			CreatedAt:  types.StringNull(),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import state mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyEarningsTargetResourceRejectsInvalidImportID(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		id   string
		want string
	}{
		{name: "missing component", id: testEarningsTargetStrategyID + "/group", want: "Import ID must have the form <strategy_id>/<ref_kind>/<ref_id>."},
		{name: "invalid strategy uuid", id: "synthetic-strategy/group/synthetic-axis/synthetic-group", want: "The first import ID component must be a strategy UUID."},
		{name: "unsupported kind", id: testEarningsTargetStrategyID + "/indicator/synthetic-id", want: "ref_kind in the import ID must be stock or group."},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			response := resource.ImportStateResponse{}
			(&strategyEarningsTargetResource{}).ImportState(context.Background(), resource.ImportStateRequest{ID: testCase.id}, &response)
			got := apiResourceDiagnosticsOutput(response.Diagnostics)
			want := []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid Import ID", Detail: testCase.want}}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("import diagnostics mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestStrategyEarningsTargetIdentityChangesRequireReplacement(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name      string
		attribute string
		oldValue  string
		newValue  string
	}{
		{name: "strategy id", attribute: "strategy_id", oldValue: testEarningsTargetStrategyID, newValue: "00000000-0000-4000-8000-000000000202"},
		{name: "reference kind", attribute: "ref_kind", oldValue: "group", newValue: "stock"},
		{name: "reference id", attribute: "ref_id", oldValue: testEarningsTargetRefID, newValue: "synthetic-stock-id"},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			ctx := context.Background()
			resourceSchema := strategyEarningsTargetTestSchema(t)
			prior := testEarningsTargetModel()
			planned := testEarningsTargetModel()
			switch testCase.attribute {
			case "strategy_id":
				prior.StrategyID = types.StringValue(testCase.oldValue)
				planned.StrategyID = types.StringValue(testCase.newValue)
			case "ref_kind":
				prior.RefKind = types.StringValue(testCase.oldValue)
				planned.RefKind = types.StringValue(testCase.newValue)
			case "ref_id":
				prior.RefID = types.StringValue(testCase.oldValue)
				planned.RefID = types.StringValue(testCase.newValue)
			}
			state := tfsdk.State{Schema: resourceSchema}
			if diagnostics := state.Set(ctx, prior); diagnostics.HasError() {
				t.Fatalf("build prior state: %v", diagnostics)
			}
			plan := tfsdk.Plan{Schema: resourceSchema}
			if diagnostics := plan.Set(ctx, planned); diagnostics.HasError() {
				t.Fatalf("build plan: %v", diagnostics)
			}
			attribute := resourceSchema.Attributes[testCase.attribute].(schema.StringAttribute)
			requiresReplace := false
			var diagnostics []apiDiagnosticObservation
			for _, modifier := range attribute.PlanModifiers {
				var response planmodifier.StringResponse
				modifier.PlanModifyString(ctx, planmodifier.StringRequest{
					Path:        path.Root(testCase.attribute),
					State:       state,
					Plan:        plan,
					StateValue:  types.StringValue(testCase.oldValue),
					PlanValue:   types.StringValue(testCase.newValue),
					ConfigValue: types.StringValue(testCase.newValue),
				}, &response)
				requiresReplace = requiresReplace || response.RequiresReplace
				diagnostics = append(diagnostics, apiResourceDiagnosticsOutput(response.Diagnostics)...)
			}

			got := struct {
				RequiresReplace bool
				Diagnostics     []apiDiagnosticObservation
			}{RequiresReplace: requiresReplace, Diagnostics: diagnostics}
			want := struct {
				RequiresReplace bool
				Diagnostics     []apiDiagnosticObservation
			}{RequiresReplace: true}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("plan modifier output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestStrategyEarningsTargetValidators(t *testing.T) {
	t.Parallel()

	resourceSchema := strategyEarningsTargetTestSchema(t)
	cases := []struct {
		name      string
		attribute string
		value     string
		want      []apiDiagnosticObservation
	}{
		{name: "stock kind is accepted", attribute: "ref_kind", value: "stock"},
		{name: "group kind is accepted", attribute: "ref_kind", value: "group"},
		{name: "unsupported kind is rejected", attribute: "ref_kind", value: "indicator", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid earnings target ref_kind", Detail: "ref_kind は stock または group にしてください。", Path: "ref_kind"}}},
		{name: "non-empty reference id is accepted", attribute: "ref_id", value: testEarningsTargetRefID},
		{name: "empty reference id is rejected", attribute: "ref_id", value: "", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid earnings target ref_id", Detail: "ref_id は空にできません。", Path: "ref_id"}}},
		{name: "padded reference id is rejected", attribute: "ref_id", value: " synthetic-id ", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid earnings target ref_id", Detail: "ref_id の前後に空白を指定できません。", Path: "ref_id"}}},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			attribute := resourceSchema.Attributes[testCase.attribute].(schema.StringAttribute)
			var response validator.StringResponse
			attribute.Validators[0].ValidateString(context.Background(), validator.StringRequest{
				Path:        path.Root(testCase.attribute),
				ConfigValue: types.StringValue(testCase.value),
			}, &response)
			got := apiResourceDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validator output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func strategyEarningsTargetTestSchema(t *testing.T) schema.Schema {
	t.Helper()
	var response resource.SchemaResponse
	(&strategyEarningsTargetResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response.Schema
}

func testEarningsTargetModel() strategyEarningsTargetModel {
	return strategyEarningsTargetModel{
		ID:         types.StringValue(strategyEarningsTargetID(testEarningsTargetStrategyID, "group", testEarningsTargetRefID)),
		StrategyID: types.StringValue(testEarningsTargetStrategyID),
		RefKind:    types.StringValue("group"),
		RefID:      types.StringValue(testEarningsTargetRefID),
		CreatedAt:  types.StringValue(testEarningsTargetCreatedAt),
	}
}

func recordEarningsTargetRequest(t *testing.T, request *http.Request) earningsTargetRequestObservation {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
	}
	return earningsTargetRequestObservation{Method: request.Method, Path: request.URL.Path, Query: request.URL.Query().Encode(), Body: string(body)}
}

func writeEarningsTargetResponse(t *testing.T, writer http.ResponseWriter, status int, body string) {
	t.Helper()
	if body != "" {
		writer.Header().Set("Content-Type", "application/json")
	}
	writer.WriteHeader(status)
	if _, err := io.WriteString(writer, body); err != nil {
		t.Errorf("write response body: %v", err)
	}
}
