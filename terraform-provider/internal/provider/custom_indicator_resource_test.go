package provider

import (
	"context"
	"encoding/json"
	"io"
	"math/big"
	"net/http"
	"net/http/httptest"
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
	"github.com/oapi-codegen/nullable"
	openapi_types "github.com/oapi-codegen/runtime/types"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

const (
	testCustomIndicatorID         = "00000000-0000-4000-8000-000000000002"
	testCustomIndicatorStrategyID = "00000000-0000-4000-8000-000000000003"
)

func TestCustomIndicatorResourceCreateRoutesByStrategyID(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name          string
		strategyID    types.String
		responseID    *string
		requestPath   string
		responseScope string
	}{
		{
			name:          "global indicator",
			strategyID:    types.StringNull(),
			requestPath:   "/api/indicators",
			responseScope: "global",
		},
		{
			name:          "strategy indicator",
			strategyID:    types.StringValue(testCustomIndicatorStrategyID),
			responseID:    stringPointer(testCustomIndicatorStrategyID),
			requestPath:   "/api/strategies/" + testCustomIndicatorStrategyID + "/indicators",
			responseScope: "strategy",
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			created := syntheticCustomIndicator(testCase.responseID)
			client, requests := newCustomIndicatorTestClient(t, func(requests chan<- customIndicatorCapturedRequest) http.Handler {
				return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
					body, err := io.ReadAll(r.Body)
					if err != nil {
						t.Errorf("read request body: %v", err)
						return
					}
					var decodedBody map[string]interface{}
					if err := json.Unmarshal(body, &decodedBody); err != nil {
						t.Errorf("decode request body: %v", err)
						return
					}
					requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path, Body: decodedBody}
					w.Header().Set("Content-Type", "application/json")
					w.WriteHeader(http.StatusCreated)
					if err := json.NewEncoder(w).Encode(created); err != nil {
						t.Errorf("encode response: %v", err)
					}
				})
			})
			resourceSchema := customIndicatorResourceSchema(t)
			plan := tfsdk.Plan{Schema: resourceSchema.Schema}
			planModel := syntheticCustomIndicatorPlan(testCase.strategyID)
			if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
				t.Fatalf("build create plan: %v", diagnostics)
			}

			response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
			(&customIndicatorResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)

			var resultState customIndicatorModel
			response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
			var observedRequest *customIndicatorCapturedRequest
			select {
			case observed := <-requests:
				observedRequest = &observed
			default:
			}

			got := struct {
				Request     *customIndicatorCapturedRequest
				State       customIndicatorModel
				Diagnostics []customIndicatorTestDiagnostic
			}{
				Request:     observedRequest,
				State:       resultState,
				Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics),
			}
			want := struct {
				Request     *customIndicatorCapturedRequest
				State       customIndicatorModel
				Diagnostics []customIndicatorTestDiagnostic
			}{
				Request: &customIndicatorCapturedRequest{
					Method: http.MethodPost,
					Path:   testCase.requestPath,
					Body: map[string]interface{}{
						"code":          "return { value: 42 }",
						"description":   "synthetic indicator description",
						"input_schema":  map[string]interface{}{"period": float64(14)},
						"name":          "synthetic indicator",
						"output_schema": map[string]interface{}{"value": float64(42)},
					},
				},
				State:       syntheticCustomIndicatorModel(testCase.responseID, testCase.responseScope),
				Diagnostics: nil,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestCustomIndicatorResourceImportStateThenRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	responseIndicator := syntheticCustomIndicator(stringPointer(testCustomIndicatorStrategyID))
	client, requests := newCustomIndicatorTestClient(t, func(requests chan<- customIndicatorCapturedRequest) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			body, err := io.ReadAll(r.Body)
			if err != nil {
				t.Errorf("read request body: %v", err)
				return
			}
			requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)}
			w.Header().Set("Content-Type", "application/json")
			if err := json.NewEncoder(w).Encode(responseIndicator); err != nil {
				t.Errorf("encode response: %v", err)
			}
		})
	})
	resourceSchema := customIndicatorResourceSchema(t)
	resourceInstance := &customIndicatorResource{client: client}
	importState := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := importState.Set(ctx, customIndicatorModel{
		ID:           types.StringNull(),
		Name:         types.StringNull(),
		Code:         types.StringNull(),
		InputSchema:  types.DynamicNull(),
		OutputSchema: types.DynamicNull(),
		Description:  types.StringNull(),
		StrategyID:   types.StringNull(),
		Scope:        types.StringNull(),
		CreatedAt:    types.StringNull(),
		UpdatedAt:    types.StringNull(),
	}); diagnostics.HasError() {
		t.Fatalf("build empty import state: %v", diagnostics)
	}
	importResponse := resource.ImportStateResponse{State: importState}
	resourceInstance.ImportState(ctx, resource.ImportStateRequest{ID: testCustomIndicatorID}, &importResponse)
	var importedID types.String
	importResponse.Diagnostics.Append(importResponse.State.GetAttribute(ctx, path.Root("id"), &importedID)...)

	readResponse := resource.ReadResponse{State: tfsdk.State{Schema: resourceSchema.Schema, Raw: importResponse.State.Raw}}
	resourceInstance.Read(ctx, resource.ReadRequest{State: importResponse.State}, &readResponse)
	var resultState customIndicatorModel
	readResponse.Diagnostics.Append(readResponse.State.Get(ctx, &resultState)...)
	var observedRequest *customIndicatorCapturedRequest
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}

	got := struct {
		ImportedID  types.String
		Request     *customIndicatorCapturedRequest
		State       customIndicatorModel
		Diagnostics []customIndicatorTestDiagnostic
	}{
		ImportedID:  importedID,
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: customIndicatorTestDiagnostics(readResponse.Diagnostics),
	}
	want := struct {
		ImportedID  types.String
		Request     *customIndicatorCapturedRequest
		State       customIndicatorModel
		Diagnostics []customIndicatorTestDiagnostic
	}{
		ImportedID: types.StringValue(testCustomIndicatorID),
		Request: &customIndicatorCapturedRequest{
			Method: http.MethodGet,
			Path:   "/api/indicators/" + testCustomIndicatorID,
			Body:   "",
		},
		State:       syntheticCustomIndicatorModel(stringPointer(testCustomIndicatorStrategyID), "strategy"),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import and read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestCustomIndicatorResourceStrategyIDRequiresReplace(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := customIndicatorResourceSchema(t)
	cases := []struct {
		name                string
		stateStrategyID     types.String
		planStrategyID      types.String
		wantRequiresReplace bool
	}{
		{
			name:                "global to strategy",
			stateStrategyID:     types.StringNull(),
			planStrategyID:      types.StringValue(testCustomIndicatorStrategyID),
			wantRequiresReplace: true,
		},
		{
			name:                "global remains global",
			stateStrategyID:     types.StringNull(),
			planStrategyID:      types.StringNull(),
			wantRequiresReplace: false,
		},
		{
			name:                "strategy remains in strategy",
			stateStrategyID:     types.StringValue(testCustomIndicatorStrategyID),
			planStrategyID:      types.StringValue(testCustomIndicatorStrategyID),
			wantRequiresReplace: false,
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			stateModel := syntheticCustomIndicatorPlan(testCase.stateStrategyID)
			stateModel.ID = types.StringValue(testCustomIndicatorID)
			stateModel.Scope = types.StringValue("global")
			if !testCase.stateStrategyID.IsNull() {
				stateModel.Scope = types.StringValue("strategy")
			}
			stateModel.CreatedAt = types.StringValue("2026-01-02T03:04:05Z")
			stateModel.UpdatedAt = types.StringValue("2026-01-02T03:04:05Z")
			state := tfsdk.State{Schema: resourceSchema.Schema}
			if diagnostics := state.Set(ctx, stateModel); diagnostics.HasError() {
				t.Fatalf("build prior state: %v", diagnostics)
			}

			planModel := syntheticCustomIndicatorPlan(testCase.planStrategyID)
			plan := tfsdk.Plan{Schema: resourceSchema.Schema}
			if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
				t.Fatalf("build replacement plan: %v", diagnostics)
			}

			strategyIDAttribute := resourceSchema.Schema.Attributes["strategy_id"].(schema.StringAttribute)
			var response planmodifier.StringResponse
			strategyIDAttribute.PlanModifiers[0].PlanModifyString(ctx, planmodifier.StringRequest{
				State:       state,
				Plan:        plan,
				StateValue:  stateModel.StrategyID,
				PlanValue:   planModel.StrategyID,
				ConfigValue: planModel.StrategyID,
			}, &response)

			got := struct {
				RequiresReplace bool
				Diagnostics     []customIndicatorTestDiagnostic
			}{RequiresReplace: response.RequiresReplace, Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics)}
			want := struct {
				RequiresReplace bool
				Diagnostics     []customIndicatorTestDiagnostic
			}{RequiresReplace: testCase.wantRequiresReplace, Diagnostics: nil}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("strategy_id plan modifier output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestCustomIndicatorNameValidator(t *testing.T) {
	t.Parallel()

	namePath := path.Root("name")
	nameAttribute := customIndicatorResourceSchema(t).Schema.Attributes["name"].(schema.StringAttribute)
	cases := []struct {
		name        string
		value       types.String
		diagnostics []customIndicatorTestDiagnostic
	}{
		{name: "null value is deferred", value: types.StringNull()},
		{name: "unknown value is deferred", value: types.StringUnknown()},
		{name: "valid name is accepted", value: types.StringValue("synthetic indicator")},
		{
			name:  "whitespace-only name is rejected",
			value: types.StringValue(" \t "),
			diagnostics: []customIndicatorTestDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid custom indicator name",
				Detail:   "Custom indicator names cannot be empty or contain only whitespace.",
			}},
		},
		{
			name:  "surrounding whitespace is rejected",
			value: types.StringValue(" synthetic indicator "),
			diagnostics: []customIndicatorTestDiagnostic{{
				Severity: diag.SeverityError,
				Summary:  "Invalid custom indicator name",
				Detail:   "Custom indicator names cannot start or end with whitespace because the API trims names.",
			}},
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			var response validator.StringResponse
			nameAttribute.Validators[0].ValidateString(context.Background(), validator.StringRequest{
				Path:        namePath,
				ConfigValue: testCase.value,
			}, &response)

			got := struct {
				Diagnostics []customIndicatorTestDiagnostic
			}{Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics)}
			want := struct {
				Diagnostics []customIndicatorTestDiagnostic
			}{Diagnostics: testCase.diagnostics}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("name validator output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestCustomIndicatorResourceUpdateNotFoundDiagnostic(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	client, requests := newCustomIndicatorTestClient(t, func(requests chan<- customIndicatorCapturedRequest) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path}
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusNotFound)
			_, _ = io.WriteString(w, `{"error":"synthetic missing indicator"}`)
		})
	})

	resourceSchema := customIndicatorResourceSchema(t)
	stateModel := syntheticCustomIndicatorModel(nil, "global")
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, stateModel); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	planModel := syntheticCustomIndicatorPlan(types.StringNull())
	planModel.Name = types.StringValue("updated synthetic indicator")
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Schema: resourceSchema.Schema, Raw: state.Raw}}
	(&customIndicatorResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var observedRequest *customIndicatorCapturedRequest
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}

	got := struct {
		Request     *customIndicatorCapturedRequest
		Diagnostics []customIndicatorTestDiagnostic
	}{Request: observedRequest, Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics)}
	want := struct {
		Request     *customIndicatorCapturedRequest
		Diagnostics []customIndicatorTestDiagnostic
	}{
		Request: &customIndicatorCapturedRequest{
			Method: http.MethodPut,
			Path:   "/api/indicators/" + testCustomIndicatorID,
		},
		Diagnostics: []customIndicatorTestDiagnostic{{
			Severity: diag.SeverityError,
			Summary:  "Error updating custom indicator",
			Detail:   "The custom indicator no longer exists.",
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update not found output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestCustomIndicatorResourceUpdate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	updatedIndicator := syntheticCustomIndicator(nil)
	updatedIndicator.Name = "updated synthetic indicator"
	updatedIndicator.Code = "return { value: 43 }"
	updatedIndicator.Description = nullable.NewNullNullable[string]()
	client, requests := newCustomIndicatorTestClient(t, func(requests chan<- customIndicatorCapturedRequest) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			body, err := io.ReadAll(r.Body)
			if err != nil {
				t.Errorf("read request body: %v", err)
				return
			}
			var decodedBody map[string]interface{}
			if err := json.Unmarshal(body, &decodedBody); err != nil {
				t.Errorf("decode request body: %v", err)
				return
			}
			requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path, Body: decodedBody}
			w.Header().Set("Content-Type", "application/json")
			if err := json.NewEncoder(w).Encode(updatedIndicator); err != nil {
				t.Errorf("encode response: %v", err)
			}
		})
	})

	resourceSchema := customIndicatorResourceSchema(t)
	stateModel := syntheticCustomIndicatorModel(nil, "global")
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, stateModel); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	planModel := syntheticCustomIndicatorPlan(types.StringNull())
	planModel.Name = types.StringValue(updatedIndicator.Name)
	planModel.Code = types.StringValue(updatedIndicator.Code)
	planModel.Description = types.StringNull()
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Schema: resourceSchema.Schema, Raw: state.Raw}}
	(&customIndicatorResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var resultState customIndicatorModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	var observedRequest *customIndicatorCapturedRequest
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}

	got := struct {
		Request     *customIndicatorCapturedRequest
		State       customIndicatorModel
		Diagnostics []customIndicatorTestDiagnostic
	}{Request: observedRequest, State: resultState, Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics)}
	wantState := syntheticCustomIndicatorModel(nil, "global")
	wantState.Name = types.StringValue(updatedIndicator.Name)
	wantState.Code = types.StringValue(updatedIndicator.Code)
	wantState.Description = types.StringNull()
	want := struct {
		Request     *customIndicatorCapturedRequest
		State       customIndicatorModel
		Diagnostics []customIndicatorTestDiagnostic
	}{
		Request: &customIndicatorCapturedRequest{
			Method: http.MethodPut,
			Path:   "/api/indicators/" + testCustomIndicatorID,
			Body: map[string]interface{}{
				"code":          updatedIndicator.Code,
				"description":   nil,
				"input_schema":  map[string]interface{}{"period": float64(14)},
				"name":          updatedIndicator.Name,
				"output_schema": map[string]interface{}{"value": float64(42)},
			},
		},
		State:       wantState,
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestCustomIndicatorResourceDelete(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name       string
		statusCode int
		body       string
	}{
		{name: "deleted indicator", statusCode: http.StatusNoContent},
		{name: "already missing indicator", statusCode: http.StatusNotFound, body: `{"error":"synthetic missing indicator"}`},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			client, requests := newCustomIndicatorTestClient(t, func(requests chan<- customIndicatorCapturedRequest) http.Handler {
				return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
					requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path}
					w.WriteHeader(testCase.statusCode)
					if testCase.body != "" {
						_, _ = io.WriteString(w, testCase.body)
					}
				})
			})

			resourceSchema := customIndicatorResourceSchema(t)
			state := tfsdk.State{Schema: resourceSchema.Schema}
			if diagnostics := state.Set(ctx, syntheticCustomIndicatorModel(nil, "global")); diagnostics.HasError() {
				t.Fatalf("build delete state: %v", diagnostics)
			}
			response := resource.DeleteResponse{}
			(&customIndicatorResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
			var observedRequest *customIndicatorCapturedRequest
			select {
			case observed := <-requests:
				observedRequest = &observed
			default:
			}

			got := struct {
				Request     *customIndicatorCapturedRequest
				Diagnostics []customIndicatorTestDiagnostic
			}{Request: observedRequest, Diagnostics: customIndicatorTestDiagnostics(response.Diagnostics)}
			want := struct {
				Request     *customIndicatorCapturedRequest
				Diagnostics []customIndicatorTestDiagnostic
			}{
				Request: &customIndicatorCapturedRequest{
					Method: http.MethodDelete,
					Path:   "/api/indicators/" + testCustomIndicatorID,
				},
				Diagnostics: nil,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestCustomIndicatorSchemaMatchesPreservesNumberDynamicType(t *testing.T) {
	t.Parallel()

	priorObject, diagnostics := types.ObjectValue(
		map[string]attr.Type{"threshold": types.NumberType},
		map[string]attr.Value{"threshold": types.NumberValue(new(big.Float).SetPrec(512).SetRat(big.NewRat(1, 10)))},
	)
	if diagnostics.HasError() {
		t.Fatalf("build prior input schema: %v", diagnostics)
	}
	prior := types.DynamicValue(priorObject)
	remote := map[string]interface{}{"threshold": float64(0.1)}

	got := struct{ Matches bool }{Matches: customIndicatorSchemaMatches(context.Background(), prior, remote)}
	want := struct{ Matches bool }{Matches: true}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("schema comparison output mismatch: got=%#v want=%#v", got, want)
	}
}

type customIndicatorCapturedRequest struct {
	Method string
	Path   string
	Body   interface{}
}

type customIndicatorTestDiagnostic struct {
	Severity diag.Severity
	Summary  string
	Detail   string
}

func customIndicatorTestDiagnostics(diagnostics diag.Diagnostics) []customIndicatorTestDiagnostic {
	result := make([]customIndicatorTestDiagnostic, 0, len(diagnostics))
	for _, diagnostic := range diagnostics {
		result = append(result, customIndicatorTestDiagnostic{
			Severity: diagnostic.Severity(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		})
	}
	if len(result) == 0 {
		return nil
	}
	return result
}

func newCustomIndicatorTestClient(
	t *testing.T,
	handler func(chan<- customIndicatorCapturedRequest) http.Handler,
) (*traderapi.Client, <-chan customIndicatorCapturedRequest) {
	t.Helper()
	requests := make(chan customIndicatorCapturedRequest, 1)
	server := httptest.NewServer(handler(requests))
	t.Cleanup(server.Close)
	return customIndicatorTestClient(t, server.URL), requests
}

func customIndicatorTestClient(t *testing.T, baseURL string) *traderapi.Client {
	t.Helper()
	client, err := traderapi.New(baseURL, "", "")
	if err != nil {
		t.Fatalf("create API client: %v", err)
	}
	return client
}

func customIndicatorResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&customIndicatorResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func syntheticCustomIndicatorPlan(strategyID types.String) customIndicatorModel {
	inputSchema := syntheticCustomIndicatorDynamic("period", 14)
	outputSchema := syntheticCustomIndicatorDynamic("value", 42)
	return customIndicatorModel{
		ID:           types.StringUnknown(),
		Name:         types.StringValue("synthetic indicator"),
		Code:         types.StringValue("return { value: 42 }"),
		InputSchema:  inputSchema,
		OutputSchema: outputSchema,
		Description:  types.StringValue("synthetic indicator description"),
		StrategyID:   strategyID,
		Scope:        types.StringUnknown(),
		CreatedAt:    types.StringUnknown(),
		UpdatedAt:    types.StringUnknown(),
	}
}

func syntheticCustomIndicatorModel(strategyID *string, scope string) customIndicatorModel {
	model := syntheticCustomIndicatorPlan(types.StringNull())
	model.ID = types.StringValue(testCustomIndicatorID)
	model.Scope = types.StringValue(scope)
	model.CreatedAt = types.StringValue("2026-01-02T03:04:05Z")
	model.UpdatedAt = types.StringValue("2026-01-03T03:04:05Z")
	if strategyID != nil {
		model.StrategyID = types.StringValue(*strategyID)
	}
	return model
}

func syntheticCustomIndicator(strategyID *string) traderapigen.CustomIndicator {
	indicator := traderapigen.CustomIndicator{
		IndicatorId: uuid.MustParse(testCustomIndicatorID),
		Name:        "synthetic indicator",
		Code:        "return { value: 42 }",
		InputSchema: map[string]interface{}{"period": float64(14)},
		OutputSchema: map[string]interface{}{
			"value": float64(42),
		},
		Description: nullable.NewNullableWithValue("synthetic indicator description"),
		Scope:       "global",
		CreatedAt:   time.Date(2026, time.January, 2, 3, 4, 5, 0, time.UTC),
		UpdatedAt:   time.Date(2026, time.January, 3, 3, 4, 5, 0, time.UTC),
		StrategyId:  nullable.NewNullNullable[openapi_types.UUID](),
	}
	if strategyID != nil {
		indicator.Scope = "strategy"
		indicator.StrategyId = nullable.NewNullableWithValue(uuid.MustParse(*strategyID))
	}
	return indicator
}

func syntheticCustomIndicatorDynamic(attributeName string, number float64) types.Dynamic {
	object := types.ObjectValueMust(
		map[string]attr.Type{attributeName: types.NumberType},
		map[string]attr.Value{attributeName: types.NumberValue(big.NewFloat(number))},
	)
	return types.DynamicValue(object)
}

func stringPointer(value string) *string {
	return &value
}
