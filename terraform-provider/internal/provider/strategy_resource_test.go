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

	"github.com/google/uuid"
	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

const testStrategyID = "00000000-0000-4000-8000-000000000001"

func TestStrategyResourceUpdateUsesPriorStateID(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	updated := traderapi.Strategy{
		Id:          uuid.MustParse(testStrategyID),
		Name:        "updated synthetic strategy",
		Description: nullable.NewNullNullable[string](),
		SortOrder:   5,
		CreatedAt:   time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		UpdatedAt:   time.Date(2026, time.January, 2, 0, 0, 0, 0, time.UTC),
	}
	requests := make(chan struct {
		Method string
		Path   string
		Body   string
	}, 1)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Errorf("read request body: %v", err)
			return
		}
		requests <- struct {
			Method string
			Path   string
			Body   string
		}{r.Method, r.URL.Path, string(body)}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		if err := json.NewEncoder(w).Encode(updated); err != nil {
			t.Errorf("encode response: %v", err)
		}
	}))
	t.Cleanup(server.Close)

	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}
	resourceSchema := strategyResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, strategyModel{
		ID:          types.StringValue(testStrategyID),
		Name:        types.StringValue("synthetic strategy"),
		Description: types.StringValue("synthetic description"),
		SortOrder:   types.Int32Value(3),
		CreatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, strategyModel{
		ID:          types.StringUnknown(),
		Name:        types.StringValue("updated synthetic strategy"),
		Description: types.StringNull(),
		SortOrder:   types.Int32Value(5),
		CreatedAt:   types.StringUnknown(),
		UpdatedAt:   types.StringUnknown(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}

	resourceInstance := &strategyResource{client: client}
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	resourceInstance.Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var resultState strategyModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	var request *struct {
		Method string
		Path   string
		Body   string
	}
	select {
	case observed := <-requests:
		request = &observed
	default:
	}

	type diagnosticOutput struct {
		Severity diag.Severity
		Summary  string
		Detail   string
	}
	type updateOutput struct {
		Request *struct {
			Method string
			Path   string
			Body   string
		}
		State       strategyModel
		Diagnostics []diagnosticOutput
	}
	var diagnostics []diagnosticOutput
	for _, diagnostic := range response.Diagnostics {
		diagnostics = append(diagnostics, diagnosticOutput{
			Severity: diagnostic.Severity(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		})
	}
	got := updateOutput{Request: request, State: resultState, Diagnostics: diagnostics}
	want := updateOutput{
		Request: &struct {
			Method string
			Path   string
			Body   string
		}{
			Method: http.MethodPatch,
			Path:   "/api/strategies/" + testStrategyID,
			Body:   `{"description":null,"name":"updated synthetic strategy","sort_order":5}`,
		},
		State: modelFromStrategy(updated),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyResourceIDPlanModifierUsesPriorState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := strategyResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, strategyModel{
		ID:          types.StringValue(testStrategyID),
		Name:        types.StringValue("synthetic strategy"),
		Description: types.StringNull(),
		SortOrder:   types.Int32Value(3),
		CreatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	idAttribute := resourceSchema.Schema.Attributes["id"].(schema.StringAttribute)
	var plannedID types.String
	for _, modifier := range idAttribute.PlanModifiers {
		var response planmodifier.StringResponse
		modifier.PlanModifyString(ctx, planmodifier.StringRequest{
			State:       state,
			StateValue:  types.StringValue(testStrategyID),
			PlanValue:   types.StringUnknown(),
			ConfigValue: types.StringNull(),
		}, &response)
		plannedID = response.PlanValue
	}
	got := struct {
		ModifierCount int
		PlannedID     types.String
	}{ModifierCount: len(idAttribute.PlanModifiers), PlannedID: plannedID}
	want := struct {
		ModifierCount int
		PlannedID     types.String
	}{ModifierCount: 1, PlannedID: types.StringValue(testStrategyID)}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("computed ID plan mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStringAttributeUpdateNullable(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		value types.String
		want  string
	}{
		{name: "unknown omits description", value: types.StringUnknown(), want: `{}`},
		{name: "null clears description", value: types.StringNull(), want: `{"description":null}`},
		{name: "value updates description", value: types.StringValue("synthetic description"), want: `{"description":"synthetic description"}`},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			request := traderapi.UpdateStrategyRequest{Description: stringAttributeUpdateNullable(testCase.value)}
			encoded, err := json.Marshal(request)
			if err != nil {
				t.Fatalf("encode update request: %v", err)
			}
			if got := string(encoded); got != testCase.want {
				t.Fatalf("update description encoding mismatch: got=%s want=%s", got, testCase.want)
			}
		})
	}
}

func TestStrategyNameValidator(t *testing.T) {
	t.Parallel()

	namePath := path.Root("name")
	cases := []struct {
		name        string
		value       types.String
		diagnostics diag.Diagnostics
	}{
		{name: "null value is deferred", value: types.StringNull()},
		{name: "unknown value is deferred", value: types.StringUnknown()},
		{name: "valid name is accepted", value: types.StringValue("synthetic strategy")},
		{
			name:  "whitespace-only name is rejected",
			value: types.StringValue(" \t "),
			diagnostics: diag.Diagnostics{diag.NewAttributeErrorDiagnostic(
				namePath,
				"Invalid strategy name",
				"Strategy names cannot be empty or contain only whitespace.",
			)},
		},
		{
			name:  "surrounding whitespace is rejected",
			value: types.StringValue(" synthetic strategy "),
			diagnostics: diag.Diagnostics{diag.NewAttributeErrorDiagnostic(
				namePath,
				"Invalid strategy name",
				"Strategy names cannot start or end with whitespace because the API trims names.",
			)},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			var response validator.StringResponse
			(strategyNameValidator{}).ValidateString(context.Background(), validator.StringRequest{
				Path:        namePath,
				ConfigValue: testCase.value,
			}, &response)
			if !response.Diagnostics.Equal(testCase.diagnostics) {
				t.Fatalf("validator diagnostics mismatch: got=%#v want=%#v", response.Diagnostics, testCase.diagnostics)
			}
		})
	}
}

func strategyResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&strategyResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}
