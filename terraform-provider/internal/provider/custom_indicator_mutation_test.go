package provider

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"
	"time"

	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"
)

func TestCustomIndicatorResourceUpdateUsesPriorStateID(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan customIndicatorCapturedRequest, 1)
	updated := syntheticCustomIndicator(stringPointer(testCustomIndicatorStrategyID))
	updated.Name = "updated synthetic indicator"
	updated.Code = "return { value: 84 }"
	updated.InputSchema = map[string]interface{}{"period": float64(30)}
	updated.OutputSchema = map[string]interface{}{"value": float64(84)}
	updated.Description = nullable.NewNullNullable[string]()
	updated.UpdatedAt = time.Date(2026, time.January, 4, 3, 4, 5, 0, time.UTC)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
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
		if err := json.NewEncoder(w).Encode(updated); err != nil {
			t.Errorf("encode response: %v", err)
		}
	}))
	t.Cleanup(server.Close)

	resourceSchema := customIndicatorResourceSchema(t)
	stateModel := syntheticCustomIndicatorModel(stringPointer(testCustomIndicatorStrategyID), "strategy")
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, stateModel); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	inputSchema := syntheticCustomIndicatorDynamic("period", 30)
	outputSchema := syntheticCustomIndicatorDynamic("value", 84)
	planModel := syntheticCustomIndicatorPlan(types.StringValue(testCustomIndicatorStrategyID))
	planModel.Name = types.StringValue("updated synthetic indicator")
	planModel.Code = types.StringValue("return { value: 84 }")
	planModel.InputSchema = inputSchema
	planModel.OutputSchema = outputSchema
	planModel.Description = types.StringNull()
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, planModel); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Schema: resourceSchema.Schema, Raw: state.Raw}}
	client := customIndicatorTestClient(t, server.URL)
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
			Method: http.MethodPut,
			Path:   "/api/indicators/" + testCustomIndicatorID,
			Body: map[string]interface{}{
				"code":          "return { value: 84 }",
				"description":   nil,
				"input_schema":  map[string]interface{}{"period": float64(30)},
				"name":          "updated synthetic indicator",
				"output_schema": map[string]interface{}{"value": float64(84)},
			},
		},
		State: customIndicatorModel{
			ID:           types.StringValue(testCustomIndicatorID),
			Name:         types.StringValue("updated synthetic indicator"),
			Code:         types.StringValue("return { value: 84 }"),
			InputSchema:  inputSchema,
			OutputSchema: outputSchema,
			Description:  types.StringNull(),
			StrategyID:   types.StringValue(testCustomIndicatorStrategyID),
			Scope:        types.StringValue("strategy"),
			CreatedAt:    types.StringValue("2026-01-02T03:04:05Z"),
			UpdatedAt:    types.StringValue("2026-01-04T03:04:05Z"),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestCustomIndicatorResourceDeleteNotFoundIsIdempotent(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan customIndicatorCapturedRequest, 1)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requests <- customIndicatorCapturedRequest{Method: r.Method, Path: r.URL.Path}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusNotFound)
		_, _ = io.WriteString(w, `{"error":"synthetic missing indicator"}`)
	}))
	t.Cleanup(server.Close)

	resourceSchema := customIndicatorResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, syntheticCustomIndicatorModel(nil, "global")); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	response := resource.DeleteResponse{}
	client := customIndicatorTestClient(t, server.URL)
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
		Diagnostics: []customIndicatorTestDiagnostic(nil),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete not found output mismatch: got=%#v want=%#v", got, want)
	}
}
