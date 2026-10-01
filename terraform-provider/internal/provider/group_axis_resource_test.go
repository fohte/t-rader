package provider

import (
	"context"
	"io"
	"net/http"
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
)

func TestGroupAxisResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
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
	observedRequest := receiveAPIResourceRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics),
	}
	want := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
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
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
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
	observedRequest := receiveAPIResourceRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{Request: observedRequest, State: resultState, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{Method: http.MethodGet, Path: "/api/group-axes/sample-axis"},
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
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
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
	observedRequest := receiveAPIResourceRequest(requests)
	var resultState groupAxisModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{Request: observedRequest, State: resultState, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       groupAxisModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
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

	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
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
	observedRequest := receiveAPIResourceRequest(requests)

	got := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: observedRequest, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{Method: http.MethodDelete, Path: "/api/group-axes/sample-axis"},
		Diagnostics: []apiDiagnosticObservation{{
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
		Diagnostics     []apiDiagnosticObservation
	}{response.PlanValue, response.RequiresReplace, apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []apiDiagnosticObservation
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
		want  []apiDiagnosticObservation
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
			got := apiResourceDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validator output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
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

func groupAxisExpectedDiagnostic(diagnosticPath path.Path, summary, detail string) []apiDiagnosticObservation {
	return []apiDiagnosticObservation{{
		Severity: diag.SeverityError.String(),
		Summary:  summary,
		Detail:   detail,
		Path:     diagnosticPath.String(),
	}}
}
