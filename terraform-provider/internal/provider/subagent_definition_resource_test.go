package provider

import (
	"context"
	"encoding/json"
	"net/http"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/hashicorp/terraform-plugin-framework/attr"
	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

const testSubagentDefinitionKey = "synthetic_subagent"

func TestSubagentDefinitionResourceValidateConfigRejectsScalarOutput(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	model := testSubagentDefinitionModel(t)
	model.Output = types.DynamicValue(types.StringValue("synthetic output"))
	resourceSchema := subagentDefinitionResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, model); diagnostics.HasError() {
		t.Fatalf("build resource config: %v", diagnostics)
	}
	config := tfsdk.Config{Raw: plan.Raw, Schema: plan.Schema}
	response := resource.ValidateConfigResponse{}

	(&subagentDefinitionResource{}).ValidateConfig(ctx, resource.ValidateConfigRequest{Config: config}, &response)

	got := apiResourceDiagnosticsOutput(response.Diagnostics)
	want := []apiDiagnosticObservation{{
		Severity: "Error",
		Summary:  "Invalid output",
		Detail:   "must be a JSON object",
		Path:     "output",
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("output validation mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceValidateConfigAcceptsPartialUnknownOutput(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	model := testSubagentDefinitionModel(t)
	model.Output = testSubagentPartiallyUnknownOutput(t)
	resourceSchema := subagentDefinitionResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, model); diagnostics.HasError() {
		t.Fatalf("build resource config: %v", diagnostics)
	}
	config := tfsdk.Config{Raw: plan.Raw, Schema: plan.Schema}
	response := resource.ValidateConfigResponse{}

	(&subagentDefinitionResource{}).ValidateConfig(ctx, resource.ValidateConfigRequest{Config: config}, &response)

	got := apiResourceDiagnosticsOutput(response.Diagnostics)
	var want []apiDiagnosticObservation
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("partial unknown output validation mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceCreateRejectsPartialUnknownOutput(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	model := testSubagentDefinitionModel(t)
	model.Output = testSubagentPartiallyUnknownOutput(t)
	resourceSchema := subagentDefinitionResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, model); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	requestSent := false
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		requestSent = true
		t.Errorf("unexpected API request: %s %s", r.Method, r.URL.Path)
	})
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)

	got := struct {
		RequestSent bool
		Diagnostics []apiDiagnosticObservation
	}{RequestSent: requestSent, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		RequestSent bool
		Diagnostics []apiDiagnosticObservation
	}{Diagnostics: []apiDiagnosticObservation{{
		Severity: "Error",
		Summary:  "Invalid output",
		Detail:   "must not contain unknown values",
		Path:     "output",
	}}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("partial unknown create mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	planModel := testSubagentDefinitionModel(t)
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeSubagentDefinitionResponse(t, w, http.StatusCreated, planModel)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, planModel)
	if planDiagnostics.HasError() {
		t.Fatalf("build create plan: %v", planDiagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	got := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	response.Diagnostics.Append(response.State.Get(ctx, &got.State)...)
	want := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/subagent-definitions",
			Body:   `{"description":"synthetic subagent","key":"synthetic_subagent","max_model_calls":4,"model":"synthetic-model","output":{"format":"json","required":["answer"]},"prompt":"synthetic instructions","reasoning_effort":"balanced","timeout_minutes":12,"tool_limits":{"fetch":2},"tools":["fetch"]}`,
		},
		State:       planModel,
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceCreateWithNumericOutputHasNoNextPlanDiff(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	planModel := testSubagentDefinitionModel(t)
	planModel.Output = testSubagentOutputWithNumber(t)
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeSubagentDefinitionResponse(t, w, http.StatusCreated, planModel)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state subagentDefinitionModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)
	nextPlan := tfsdk.Plan{Schema: resourceSchema.Schema}
	response.Diagnostics.Append(nextPlan.Set(ctx, planModel)...)

	got := struct {
		Request              *apiRequestObservation
		State                subagentDefinitionModel
		NextPlanMatchesState bool
		Diagnostics          []apiDiagnosticObservation
	}{
		Request:              receiveAPIResourceRequest(requests),
		State:                state,
		NextPlanMatchesState: response.State.Raw.Equal(nextPlan.Raw),
		Diagnostics:          apiResourceDiagnosticsOutput(response.Diagnostics),
	}
	want := struct {
		Request              *apiRequestObservation
		State                subagentDefinitionModel
		NextPlanMatchesState bool
		Diagnostics          []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/subagent-definitions",
			Body:   `{"description":"synthetic subagent","key":"synthetic_subagent","max_model_calls":4,"model":"synthetic-model","output":{"format":"json","threshold":0.75},"prompt":"synthetic instructions","reasoning_effort":"balanced","timeout_minutes":12,"tool_limits":{"fetch":2},"tools":["fetch"]}`,
		},
		State:                planModel,
		NextPlanMatchesState: true,
		Diagnostics:          nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("numeric output create mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	model := testSubagentDefinitionModel(t)
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeSubagentDefinitionResponse(t, w, http.StatusOK, model)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, model)
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var result subagentDefinitionModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)
	got := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), State: result, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request:     &apiRequestObservation{Method: http.MethodGet, Path: "/api/subagent-definitions/" + testSubagentDefinitionKey},
		State:       model,
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceReadRemovesMissingDefinition(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeAPIResourceResponse(t, w, http.StatusNotFound, `{"error":"synthetic missing definition"}`)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, testSubagentDefinitionModel(t))
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	got := struct {
		Request      *apiRequestObservation
		StateRemoved bool
		Diagnostics  []apiDiagnosticObservation
	}{
		Request:      receiveAPIResourceRequest(requests),
		StateRemoved: response.State.Raw.IsNull(),
		Diagnostics:  apiResourceDiagnosticsOutput(response.Diagnostics),
	}
	want := struct {
		Request      *apiRequestObservation
		StateRemoved bool
		Diagnostics  []apiDiagnosticObservation
	}{
		Request:      &apiRequestObservation{Method: http.MethodGet, Path: "/api/subagent-definitions/" + testSubagentDefinitionKey},
		StateRemoved: true,
		Diagnostics:  nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read missing output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceUpdateClearsOptionalReasoningEffort(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	priorModel := testSubagentDefinitionModel(t)
	planModel := testSubagentDefinitionModel(t)
	planModel.Description = types.StringValue("updated synthetic description")
	planModel.ReasoningEffort = types.StringNull()
	planModel.Prompt = types.StringValue("updated synthetic instructions")
	planModel.TimeoutMinutes = types.Int32Value(24)
	planModel.MaxModelCalls = types.Int64Value(6)
	planModel.Tools = testSubagentTools(t, []string{"fetch", "lookup"})
	planModel.ToolLimits = testSubagentToolLimits(t, map[string]int64{"fetch": 3, "lookup": 1})
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeSubagentDefinitionResponse(t, w, http.StatusOK, planModel)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, priorModel)
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, planModel)
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var result subagentDefinitionModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)
	got := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), State: result, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPut,
			Path:   "/api/subagent-definitions/" + testSubagentDefinitionKey,
			Body:   `{"description":"updated synthetic description","max_model_calls":6,"model":"synthetic-model","output":{"format":"json","required":["answer"]},"prompt":"updated synthetic instructions","reasoning_effort":null,"timeout_minutes":24,"tool_limits":{"fetch":3,"lookup":1},"tools":["fetch","lookup"]}`,
		},
		State:       planModel,
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceUpdateReturnsBackendValidationDiagnostic(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	model := testSubagentDefinitionModel(t)
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeAPIResourceResponse(t, w, http.StatusBadRequest, `{"error":"prompt must not be blank"}`)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, model)
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, model)
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}

	(&subagentDefinitionResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	got := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPut,
			Path:   "/api/subagent-definitions/" + testSubagentDefinitionKey,
			Body:   `{"description":"synthetic subagent","max_model_calls":4,"model":"synthetic-model","output":{"format":"json","required":["answer"]},"prompt":"synthetic instructions","reasoning_effort":"balanced","timeout_minutes":12,"tool_limits":{"fetch":2},"tools":["fetch"]}`,
		},
		Diagnostics: []apiDiagnosticObservation{{
			Severity: "Error",
			Summary:  "Error updating subagent definition",
			Detail:   "backend returned HTTP 400: prompt must not be blank",
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("validation diagnostic mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceDeleteReturnsReferenceConflictDiagnostic(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeAPIResourceResponse(t, w, http.StatusConflict, `{"error":"definition is referenced by an agent graph"}`)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, testSubagentDefinitionModel(t))
	if stateDiagnostics.HasError() {
		t.Fatalf("build delete state: %v", stateDiagnostics)
	}
	response := resource.DeleteResponse{}

	(&subagentDefinitionResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	got := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{
		Request:     &apiRequestObservation{Method: http.MethodDelete, Path: "/api/subagent-definitions/" + testSubagentDefinitionKey},
		Diagnostics: []apiDiagnosticObservation{{Severity: "Error", Summary: "Error deleting subagent definition", Detail: "backend returned HTTP 409: definition is referenced by an agent graph"}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete conflict output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	resourceSchema := subagentDefinitionResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, testSubagentDefinitionModel(t))
	if stateDiagnostics.HasError() {
		t.Fatalf("build delete state: %v", stateDiagnostics)
	}
	response := resource.DeleteResponse{}

	(&subagentDefinitionResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	got := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: receiveAPIResourceRequest(requests), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: &apiRequestObservation{Method: http.MethodDelete, Path: "/api/subagent-definitions/" + testSubagentDefinitionKey}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := subagentDefinitionResourceSchema(t)
	response := resource.ImportStateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	stateDiagnostics := response.State.Set(ctx, subagentDefinitionNullModel())
	if stateDiagnostics.HasError() {
		t.Fatalf("build import state: %v", stateDiagnostics)
	}

	(&subagentDefinitionResource{}).ImportState(ctx, resource.ImportStateRequest{ID: testSubagentDefinitionKey}, &response)
	var imported subagentDefinitionModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)
	got := struct {
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{State: imported, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       subagentDefinitionModel
		Diagnostics []apiDiagnosticObservation
	}{State: subagentDefinitionNullModel(), Diagnostics: nil}
	want.State.Key = types.StringValue(testSubagentDefinitionKey)
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceKeyRequiresReplacement(t *testing.T) {
	t.Parallel()

	resourceSchema := subagentDefinitionResourceSchema(t)
	attribute := resourceSchema.Schema.Attributes["key"].(schema.StringAttribute)
	ctx := context.Background()
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, testSubagentDefinitionModel(t))
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planModel := testSubagentDefinitionModel(t)
	planModel.Key = types.StringValue("renamed_synthetic_subagent")
	planDiagnostics := plan.Set(ctx, planModel)
	if planDiagnostics.HasError() {
		t.Fatalf("build plan: %v", planDiagnostics)
	}
	request := planmodifier.StringRequest{
		Path:        path.Root("key"),
		State:       state,
		Plan:        plan,
		StateValue:  types.StringValue(testSubagentDefinitionKey),
		PlanValue:   planModel.Key,
		ConfigValue: planModel.Key,
	}
	response := planmodifier.StringResponse{PlanValue: request.PlanValue}
	attribute.PlanModifiers[0].PlanModifyString(ctx, request, &response)

	got := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []apiDiagnosticObservation
	}{PlanValue: response.PlanValue, RequiresReplace: response.RequiresReplace, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []apiDiagnosticObservation
	}{PlanValue: types.StringValue("renamed_synthetic_subagent"), RequiresReplace: true}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("key plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestSubagentDefinitionResourceKeyValidator(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		key  string
		want []apiDiagnosticObservation
	}{
		{name: "accepts lowercase key", key: testSubagentDefinitionKey},
		{name: "rejects uppercase key", key: "Synthetic_subagent", want: []apiDiagnosticObservation{{
			Severity: "Error",
			Summary:  "Invalid subagent definition key",
			Detail:   "Key must match ^[a-z0-9][a-z0-9_-]*$.",
			Path:     "key",
		}}},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			request := validator.StringRequest{Path: path.Root("key"), ConfigValue: types.StringValue(testCase.key)}
			response := validator.StringResponse{}
			subagentDefinitionKeyValidator{}.ValidateString(context.Background(), request, &response)
			got := apiResourceDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("key validation mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func TestSubagentDefinitionResourceNumericValidators(t *testing.T) {
	t.Parallel()

	resourceSchema := subagentDefinitionResourceSchema(t)
	timeoutAttribute := resourceSchema.Schema.Attributes["timeout_minutes"].(schema.Int32Attribute)
	maxCallsAttribute := resourceSchema.Schema.Attributes["max_model_calls"].(schema.Int64Attribute)
	cases := []struct {
		name     string
		timeout  types.Int32
		maxCalls types.Int64
		want     []apiDiagnosticObservation
	}{
		{name: "accepts minimums", timeout: types.Int32Value(1), maxCalls: types.Int64Value(1)},
		{name: "accepts timeout maximum", timeout: types.Int32Value(360), maxCalls: types.Int64Value(1)},
		{name: "rejects timeout below minimum", timeout: types.Int32Value(0), maxCalls: types.Int64Value(1), want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid timeout_minutes", Detail: "timeout_minutes must be between 1 and 360.", Path: "timeout_minutes"}}},
		{name: "rejects timeout above maximum", timeout: types.Int32Value(361), maxCalls: types.Int64Value(1), want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid timeout_minutes", Detail: "timeout_minutes must be between 1 and 360.", Path: "timeout_minutes"}}},
		{name: "rejects nonpositive model calls", timeout: types.Int32Value(1), maxCalls: types.Int64Value(0), want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid max_model_calls", Detail: "max_model_calls must be at least 1.", Path: "max_model_calls"}}},
		{name: "accepts null values", timeout: types.Int32Null(), maxCalls: types.Int64Null()},
		{name: "accepts unknown values", timeout: types.Int32Unknown(), maxCalls: types.Int64Unknown()},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			var diagnostics diag.Diagnostics
			for _, valueValidator := range timeoutAttribute.Validators {
				response := validator.Int32Response{}
				valueValidator.ValidateInt32(context.Background(), validator.Int32Request{Path: path.Root("timeout_minutes"), ConfigValue: testCase.timeout}, &response)
				diagnostics.Append(response.Diagnostics...)
			}
			for _, valueValidator := range maxCallsAttribute.Validators {
				response := validator.Int64Response{}
				valueValidator.ValidateInt64(context.Background(), validator.Int64Request{Path: path.Root("max_model_calls"), ConfigValue: testCase.maxCalls}, &response)
				diagnostics.Append(response.Diagnostics...)
			}
			got := apiResourceDiagnosticsOutput(diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("numeric validation mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func subagentDefinitionResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&subagentDefinitionResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func testSubagentDefinitionModel(t *testing.T) subagentDefinitionModel {
	t.Helper()
	return subagentDefinitionModel{
		Key:             types.StringValue(testSubagentDefinitionKey),
		Description:     types.StringValue("synthetic subagent"),
		Model:           types.StringValue("synthetic-model"),
		ReasoningEffort: types.StringValue("balanced"),
		Prompt:          types.StringValue("synthetic instructions"),
		Tools:           testSubagentTools(t, []string{"fetch"}),
		TimeoutMinutes:  types.Int32Value(12),
		MaxModelCalls:   types.Int64Value(4),
		ToolLimits:      testSubagentToolLimits(t, map[string]int64{"fetch": 2}),
		Output:          testSubagentOutput(t),
	}
}

func subagentDefinitionNullModel() subagentDefinitionModel {
	return subagentDefinitionModel{
		Key:             types.StringNull(),
		Description:     types.StringNull(),
		Model:           types.StringNull(),
		ReasoningEffort: types.StringNull(),
		Prompt:          types.StringNull(),
		Tools:           types.ListNull(types.StringType),
		TimeoutMinutes:  types.Int32Null(),
		MaxModelCalls:   types.Int64Null(),
		ToolLimits:      types.MapNull(types.Int64Type),
		Output:          types.DynamicNull(),
	}
}

func testSubagentTools(t *testing.T, tools []string) types.List {
	t.Helper()
	value, diagnostics := types.ListValueFrom(context.Background(), types.StringType, tools)
	if diagnostics.HasError() {
		t.Fatalf("build tools: %v", diagnostics)
	}
	return value
}

func testSubagentToolLimits(t *testing.T, limits map[string]int64) types.Map {
	t.Helper()
	value, diagnostics := types.MapValueFrom(context.Background(), types.Int64Type, limits)
	if diagnostics.HasError() {
		t.Fatalf("build tool limits: %v", diagnostics)
	}
	return value
}

func testSubagentOutput(t *testing.T) types.Dynamic {
	t.Helper()
	value, err := dynamicFromJSON(map[string]interface{}{
		"format":   "json",
		"required": []interface{}{"answer"},
	})
	if err != nil {
		t.Fatalf("build output: %v", err)
	}
	return value
}

func testSubagentOutputWithNumber(t *testing.T) types.Dynamic {
	t.Helper()
	value, err := dynamicFromJSON(map[string]interface{}{
		"format":    "json",
		"threshold": float64(0.75),
	})
	if err != nil {
		t.Fatalf("build output with a number: %v", err)
	}
	return value
}

func testSubagentPartiallyUnknownOutput(t *testing.T) types.Dynamic {
	t.Helper()
	value, diagnostics := types.ObjectValue(
		map[string]attr.Type{
			"format":   types.StringType,
			"required": types.ListType{ElemType: types.StringType},
		},
		map[string]attr.Value{
			"format":   types.StringValue("json"),
			"required": types.ListUnknown(types.StringType),
		},
	)
	if diagnostics.HasError() {
		t.Fatalf("build partially unknown output: %v", diagnostics)
	}
	return types.DynamicValue(value)
}

func writeSubagentDefinitionResponse(t *testing.T, w http.ResponseWriter, status int, model subagentDefinitionModel) {
	t.Helper()
	var tools []string
	if diagnostics := model.Tools.ElementsAs(context.Background(), &tools, false); diagnostics.HasError() {
		t.Errorf("convert tools for response: %v", diagnostics)
		return
	}
	var toolLimits map[string]int64
	if diagnostics := model.ToolLimits.ElementsAs(context.Background(), &toolLimits, false); diagnostics.HasError() {
		t.Errorf("convert tool limits for response: %v", diagnostics)
		return
	}
	output, err := jsonObjectFromDynamic(context.Background(), model.Output)
	if err != nil {
		t.Errorf("convert output for response: %v", err)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(gen.SubagentDefinitionResponse{
		CreatedAt:       time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC),
		Description:     model.Description.ValueString(),
		Id:              uuid.MustParse("00000000-0000-4000-8000-000000000003"),
		Key:             model.Key.ValueString(),
		MaxModelCalls:   model.MaxModelCalls.ValueInt64(),
		Model:           model.Model.ValueString(),
		Output:          output,
		Prompt:          model.Prompt.ValueString(),
		ReasoningEffort: stringAttributeUpdateNullable(model.ReasoningEffort),
		TimeoutMinutes:  model.TimeoutMinutes.ValueInt32(),
		ToolLimits:      toolLimits,
		Tools:           tools,
		UpdatedAt:       time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC),
	}); err != nil {
		t.Errorf("encode subagent definition response: %v", err)
	}
}
