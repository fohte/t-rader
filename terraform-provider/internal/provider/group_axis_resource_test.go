package provider

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

type groupAxisRequestObservation struct {
	Method string
	Path   string
	Body   string
}

type groupAxisDiagnosticObservation struct {
	Severity string
	Summary  string
	Detail   string
	Path     string
}

func TestGroupAxisResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan groupAxisRequestObservation, 1)
	client := newGroupAxisTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordGroupAxisRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		writeGroupAxisResponse(t, w, `{"key":"sample-axis","name":"Sample Axis","description":"A synthetic classification axis","sync_source":null}`)
	})
	resourceSchema := groupAxisResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Sample Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringNull(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build create plan: %v", planDiagnostics)
	}

	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	(&groupAxisResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	observedRequest := receiveGroupAxisRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: groupAxisDiagnosticsOutput(response.Diagnostics),
	}
	want := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{
		Request: &groupAxisRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/group-axes",
			Body:   `{"description":"A synthetic classification axis","key":"sample-axis","name":"Sample Axis"}`,
		},
		State: groupAxisModel{
			Key:         types.StringValue("sample-axis"),
			Name:        types.StringValue("Sample Axis"),
			Description: types.StringValue("A synthetic classification axis"),
			SyncSource:  types.StringNull(),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestGroupAxisResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan groupAxisRequestObservation, 1)
	client := newGroupAxisTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordGroupAxisRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		writeGroupAxisResponse(t, w, `{"key":"sample-axis","name":"Refreshed Axis","description":"A synthetic classification axis","sync_source":"sample-source"}`)
	})
	resourceSchema := groupAxisResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Stale Axis"),
		Description: types.StringValue("A stale description"),
		SyncSource:  types.StringNull(),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}

	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}
	(&groupAxisResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	observedRequest := receiveGroupAxisRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{Request: observedRequest, State: resultState, Diagnostics: groupAxisDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{
		Request: &groupAxisRequestObservation{Method: http.MethodGet, Path: "/api/group-axes/sample-axis"},
		State: groupAxisModel{
			Key:         types.StringValue("sample-axis"),
			Name:        types.StringValue("Refreshed Axis"),
			Description: types.StringValue("A synthetic classification axis"),
			SyncSource:  types.StringValue("sample-source"),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestGroupAxisResourceUpdateClearsSyncSource(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan groupAxisRequestObservation, 1)
	client := newGroupAxisTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordGroupAxisRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		writeGroupAxisResponse(t, w, `{"key":"sample-axis","name":"Updated Axis","description":"A synthetic classification axis","sync_source":null}`)
	})
	resourceSchema := groupAxisResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Sample Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringValue("sample-source"),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Updated Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringNull(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&groupAxisResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	observedRequest := receiveGroupAxisRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{Request: observedRequest, State: resultState, Diagnostics: groupAxisDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *groupAxisRequestObservation
		State       groupAxisModel
		Diagnostics []groupAxisDiagnosticObservation
	}{
		Request: &groupAxisRequestObservation{
			Method: http.MethodPatch,
			Path:   "/api/group-axes/sample-axis",
			Body:   `{"description":"A synthetic classification axis","name":"Updated Axis","sync_source":null}`,
		},
		State: groupAxisModel{
			Key:         types.StringValue("sample-axis"),
			Name:        types.StringValue("Updated Axis"),
			Description: types.StringValue("A synthetic classification axis"),
			SyncSource:  types.StringNull(),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestGroupAxisResourceDeleteReportsConflict(t *testing.T) {
	t.Parallel()

	requests := make(chan groupAxisRequestObservation, 1)
	client := newGroupAxisTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordGroupAxisRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusConflict)
		writeGroupAxisResponse(t, w, `{"error":"group axis sample-axis cannot be deleted while it contains groups"}`)
	})
	resourceSchema := groupAxisResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(context.Background(), groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Sample Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringNull(),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}

	response := resource.DeleteResponse{State: state}
	(&groupAxisResource{client: client}).Delete(context.Background(), resource.DeleteRequest{State: state}, &response)
	observedRequest := receiveGroupAxisRequest(requests)

	got := struct {
		Request     *groupAxisRequestObservation
		Diagnostics []groupAxisDiagnosticObservation
	}{Request: observedRequest, Diagnostics: groupAxisDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *groupAxisRequestObservation
		Diagnostics []groupAxisDiagnosticObservation
	}{
		Request: &groupAxisRequestObservation{Method: http.MethodDelete, Path: "/api/group-axes/sample-axis"},
		Diagnostics: []groupAxisDiagnosticObservation{{
			Severity: diag.SeverityError.String(),
			Summary:  "Error deleting group axis",
			Detail:   `backend returned HTTP 409: {"error":"group axis sample-axis cannot be deleted while it contains groups"}`,
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestGroupAxisResourceKeyChangeRequiresReplacement(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := groupAxisResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, groupAxisModel{
		Key:         types.StringValue("sample-axis"),
		Name:        types.StringValue("Sample Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringNull(),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, groupAxisModel{
		Key:         types.StringValue("replacement-axis"),
		Name:        types.StringValue("Sample Axis"),
		Description: types.StringValue("A synthetic classification axis"),
		SyncSource:  types.StringNull(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build replacement plan: %v", planDiagnostics)
	}
	attribute := resourceSchema.Schema.Attributes["key"].(schema.StringAttribute)
	request := planmodifier.StringRequest{
		Path:        path.Root("key"),
		State:       state,
		Plan:        plan,
		StateValue:  types.StringValue("sample-axis"),
		PlanValue:   types.StringValue("replacement-axis"),
		ConfigValue: types.StringValue("replacement-axis"),
	}
	response := planmodifier.StringResponse{PlanValue: request.PlanValue}
	attribute.PlanModifiers[0].PlanModifyString(ctx, request, &response)

	got := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []groupAxisDiagnosticObservation
	}{response.PlanValue, response.RequiresReplace, groupAxisDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []groupAxisDiagnosticObservation
	}{types.StringValue("replacement-axis"), true, nil}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestGroupAxisResourceKeyValidator(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		value types.String
		want  []groupAxisDiagnosticObservation
	}{
		{name: "accepts a key without path separators", value: types.StringValue("sample-axis")},
		{
			name:  "rejects path separators",
			value: types.StringValue("sample/axis"),
			want:  groupAxisExpectedDiagnostic(path.Root("key"), "Invalid group axis key", "Key must not be empty, contain '/', or have surrounding whitespace."),
		},
		{
			name:  "rejects surrounding whitespace",
			value: types.StringValue(" sample-axis "),
			want:  groupAxisExpectedDiagnostic(path.Root("key"), "Invalid group axis key", "Key must not be empty, contain '/', or have surrounding whitespace."),
		},
	}
	resourceSchema := groupAxisResourceSchema(t)
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			attribute := resourceSchema.Schema.Attributes["key"].(schema.StringAttribute)
			var response validator.StringResponse
			attribute.Validators[0].ValidateString(context.Background(), validator.StringRequest{
				Path:        path.Root("key"),
				ConfigValue: testCase.value,
			}, &response)
			got := groupAxisDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validator output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func newGroupAxisTestClient(t *testing.T, handler http.HandlerFunc) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}
	return client
}

func recordGroupAxisRequest(t *testing.T, requests chan<- groupAxisRequestObservation, request *http.Request) bool {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
		return false
	}
	requests <- groupAxisRequestObservation{Method: request.Method, Path: request.URL.Path, Body: string(body)}
	return true
}

func receiveGroupAxisRequest(requests <-chan groupAxisRequestObservation) *groupAxisRequestObservation {
	observed := <-requests
	return &observed
}

func writeGroupAxisResponse(t *testing.T, writer io.Writer, body string) {
	t.Helper()
	if _, err := io.WriteString(writer, body); err != nil {
		t.Errorf("write group axis response: %v", err)
	}
}

func groupAxisResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&groupAxisResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func groupAxisExpectedDiagnostic(diagnosticPath path.Path, summary, detail string) []groupAxisDiagnosticObservation {
	return []groupAxisDiagnosticObservation{{
		Severity: diag.SeverityError.String(),
		Summary:  summary,
		Detail:   detail,
		Path:     diagnosticPath.String(),
	}}
}

func groupAxisDiagnosticsOutput(diagnostics diag.Diagnostics) []groupAxisDiagnosticObservation {
	var result []groupAxisDiagnosticObservation
	for _, diagnostic := range diagnostics {
		observation := groupAxisDiagnosticObservation{
			Severity: diagnostic.Severity().String(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		}
		if diagnosticWithPath, ok := diagnostic.(diag.DiagnosticWithPath); ok {
			observation.Path = diagnosticWithPath.Path().String()
		}
		result = append(result, observation)
	}
	return result
}
