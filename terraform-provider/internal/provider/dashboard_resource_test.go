package provider

import (
	"context"
	"encoding/json"
	"net/http"
	"reflect"
	"testing"
	"time"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"

	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

const (
	testDashboardKey    = "sample-dashboard"
	testDashboardConfig = "title: sample-dashboard\nwidgets: []\n"
	testUpdatedConfig   = "title: updated-dashboard\nwidgets: []\n"
)

func TestDashboardResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeAPIResourceResponse(t, w, http.StatusCreated, dashboardResponseJSON(t, testDashboardKey, testDashboardConfig))
	})
	resourceSchema := dashboardTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, dashboardTestModel(testDashboardKey, testDashboardConfig)); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}

	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema}}
	(&dashboardResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	observedRequest := receiveAPIResourceRequest(requests)
	var resultState dashboardModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *apiRequestObservation
		State       dashboardModel
		Diagnostics []apiDiagnosticObservation
	}{observedRequest, resultState, apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       dashboardModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/dashboards",
			Body:   `{"config":"title: sample-dashboard\nwidgets: []\n","key":"sample-dashboard"}`,
		},
		State:       dashboardTestModel(testDashboardKey, testDashboardConfig),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestDashboardResourceRead(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name          string
		statusCode    int
		body          string
		wantState     dashboardModel
		wantStateNull bool
	}{
		{
			name:       "refreshes state from the API",
			statusCode: http.StatusOK,
			body:       dashboardResponseJSON(t, testDashboardKey, testDashboardConfig),
			wantState:  dashboardTestModel(testDashboardKey, testDashboardConfig),
		},
		{
			name:          "removes a missing dashboard from state",
			statusCode:    http.StatusNotFound,
			body:          `{"error":"dashboard is missing"}`,
			wantStateNull: true,
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			requests := make(chan apiRequestObservation, 1)
			client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				if !recordAPIResourceRequest(t, requests, r) {
					return
				}
				writeAPIResourceResponse(t, w, testCase.statusCode, testCase.body)
			})
			state := dashboardTestState(t, dashboardTestModel(testDashboardKey, "title: stale-dashboard\nwidgets: []\n"))

			response := resource.ReadResponse{State: state}
			(&dashboardResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
			observedRequest := receiveAPIResourceRequest(requests)
			var resultState dashboardModel
			if !response.State.Raw.IsNull() {
				response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
			}

			got := struct {
				Request     *apiRequestObservation
				State       dashboardModel
				StateIsNull bool
				Diagnostics []apiDiagnosticObservation
			}{observedRequest, resultState, response.State.Raw.IsNull(), apiResourceDiagnosticsOutput(response.Diagnostics)}
			want := struct {
				Request     *apiRequestObservation
				State       dashboardModel
				StateIsNull bool
				Diagnostics []apiDiagnosticObservation
			}{
				Request:     &apiRequestObservation{Method: http.MethodGet, Path: "/api/dashboards/" + testDashboardKey},
				State:       testCase.wantState,
				StateIsNull: testCase.wantStateNull,
				Diagnostics: nil,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestDashboardResourceUpdate(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name            string
		statusCode      int
		body            string
		wantState       dashboardModel
		wantDiagnostics []apiDiagnosticObservation
	}{
		{
			name:       "updates an existing dashboard",
			statusCode: http.StatusOK,
			body:       dashboardResponseJSON(t, testDashboardKey, testUpdatedConfig),
			wantState:  dashboardTestModel(testDashboardKey, testUpdatedConfig),
		},
		{
			name:       "reports a missing dashboard",
			statusCode: http.StatusNotFound,
			body:       `{"error":"dashboard is missing"}`,
			wantState:  dashboardTestModel(testDashboardKey, testDashboardConfig),
			wantDiagnostics: []apiDiagnosticObservation{{
				Severity: diag.SeverityError.String(),
				Summary:  "Error updating dashboard",
				Detail:   "The dashboard no longer exists.",
			}},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			requests := make(chan apiRequestObservation, 1)
			client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				if !recordAPIResourceRequest(t, requests, r) {
					return
				}
				writeAPIResourceResponse(t, w, testCase.statusCode, testCase.body)
			})
			resourceSchema := dashboardTestSchema(t)
			priorState := dashboardTestModel(testDashboardKey, testDashboardConfig)
			state := dashboardTestState(t, priorState)
			plan := tfsdk.Plan{Schema: resourceSchema}
			if diagnostics := plan.Set(ctx, dashboardTestModel(testDashboardKey, testUpdatedConfig)); diagnostics.HasError() {
				t.Fatalf("build update plan: %v", diagnostics)
			}

			response := resource.UpdateResponse{State: state}
			(&dashboardResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
			observedRequest := receiveAPIResourceRequest(requests)
			var resultState dashboardModel
			response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

			got := struct {
				Request     *apiRequestObservation
				State       dashboardModel
				Diagnostics []apiDiagnosticObservation
			}{observedRequest, resultState, apiResourceDiagnosticsOutput(response.Diagnostics)}
			want := struct {
				Request     *apiRequestObservation
				State       dashboardModel
				Diagnostics []apiDiagnosticObservation
			}{
				Request: &apiRequestObservation{
					Method: http.MethodPut,
					Path:   "/api/dashboards/" + testDashboardKey,
					Body:   `{"config":"title: updated-dashboard\nwidgets: []\n"}`,
				},
				State:       testCase.wantState,
				Diagnostics: testCase.wantDiagnostics,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestDashboardResourceDelete(t *testing.T) {
	t.Parallel()

	for _, testCase := range []struct {
		name       string
		statusCode int
		body       string
	}{
		{name: "deletes an existing dashboard", statusCode: http.StatusNoContent},
		{name: "accepts a missing dashboard", statusCode: http.StatusNotFound, body: `{"error":"dashboard is missing"}`},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			requests := make(chan apiRequestObservation, 1)
			client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				if !recordAPIResourceRequest(t, requests, r) {
					return
				}
				writeAPIResourceResponse(t, w, testCase.statusCode, testCase.body)
			})
			state := dashboardTestState(t, dashboardTestModel(testDashboardKey, testDashboardConfig))

			var response resource.DeleteResponse
			(&dashboardResource{client: client}).Delete(context.Background(), resource.DeleteRequest{State: state}, &response)
			observedRequest := receiveAPIResourceRequest(requests)

			got := struct {
				Request     *apiRequestObservation
				Diagnostics []apiDiagnosticObservation
			}{observedRequest, apiResourceDiagnosticsOutput(response.Diagnostics)}
			want := struct {
				Request     *apiRequestObservation
				Diagnostics []apiDiagnosticObservation
			}{Request: &apiRequestObservation{Method: http.MethodDelete, Path: "/api/dashboards/" + testDashboardKey}}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestDashboardResourceImportStateThenRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		writeAPIResourceResponse(t, w, http.StatusOK, dashboardResponseJSON(t, testDashboardKey, testDashboardConfig))
	})
	resourceSchema := dashboardTestSchema(t)
	resourceInstance := &dashboardResource{client: client}
	importState := tfsdk.State{Schema: resourceSchema}
	if diagnostics := importState.Set(ctx, dashboardModel{Key: types.StringNull(), Config: types.StringNull()}); diagnostics.HasError() {
		t.Fatalf("build empty import state: %v", diagnostics)
	}
	importResponse := resource.ImportStateResponse{State: importState}
	resourceInstance.ImportState(ctx, resource.ImportStateRequest{ID: testDashboardKey}, &importResponse)
	var importedKey types.String
	importResponse.Diagnostics.Append(importResponse.State.GetAttribute(ctx, path.Root("key"), &importedKey)...)

	readResponse := resource.ReadResponse{State: tfsdk.State{Schema: resourceSchema, Raw: importResponse.State.Raw}}
	resourceInstance.Read(ctx, resource.ReadRequest{State: importResponse.State}, &readResponse)
	observedRequest := receiveAPIResourceRequest(requests)
	var resultState dashboardModel
	readResponse.Diagnostics.Append(readResponse.State.Get(ctx, &resultState)...)

	got := struct {
		ImportedKey types.String
		Request     *apiRequestObservation
		State       dashboardModel
		Diagnostics []apiDiagnosticObservation
	}{importedKey, observedRequest, resultState, apiResourceDiagnosticsOutput(readResponse.Diagnostics)}
	want := struct {
		ImportedKey types.String
		Request     *apiRequestObservation
		State       dashboardModel
		Diagnostics []apiDiagnosticObservation
	}{
		ImportedKey: types.StringValue(testDashboardKey),
		Request:     &apiRequestObservation{Method: http.MethodGet, Path: "/api/dashboards/" + testDashboardKey},
		State:       dashboardTestModel(testDashboardKey, testDashboardConfig),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import and read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestDashboardResourceKeyChangeRequiresReplacement(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := dashboardTestSchema(t)
	state := dashboardTestState(t, dashboardTestModel(testDashboardKey, testDashboardConfig))
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, dashboardTestModel("replacement-dashboard", testDashboardConfig)); diagnostics.HasError() {
		t.Fatalf("build replacement plan: %v", diagnostics)
	}
	attribute := resourceSchema.Attributes["key"].(schema.StringAttribute)
	request := planmodifier.StringRequest{
		Path:        path.Root("key"),
		State:       state,
		Plan:        plan,
		StateValue:  types.StringValue(testDashboardKey),
		PlanValue:   types.StringValue("replacement-dashboard"),
		ConfigValue: types.StringValue("replacement-dashboard"),
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
	}{types.StringValue("replacement-dashboard"), true, nil}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestDashboardResourceKeyValidator(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		value string
		want  []apiDiagnosticObservation
	}{
		{name: "accepts a dashboard key", value: testDashboardKey},
		{name: "rejects an empty key", value: "", want: dashboardExpectedDiagnostic("Key must not be empty or have surrounding whitespace.")},
		{name: "rejects surrounding whitespace", value: " sample-dashboard ", want: dashboardExpectedDiagnostic("Key must not be empty or have surrounding whitespace.")},
	}
	resourceSchema := dashboardTestSchema(t)
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			attribute := resourceSchema.Attributes["key"].(schema.StringAttribute)
			var response validator.StringResponse
			attribute.Validators[0].ValidateString(context.Background(), validator.StringRequest{
				Path:        path.Root("key"),
				ConfigValue: types.StringValue(testCase.value),
			}, &response)
			got := apiResourceDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validator output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func TestProviderRegistersDashboardResource(t *testing.T) {
	t.Parallel()

	type output struct {
		Registered bool
		TypeName   string
	}
	got := output{}
	for _, factory := range (&traderProvider{}).Resources(context.Background()) {
		if dashboard, ok := factory().(*dashboardResource); ok {
			got.Registered = true
			var response resource.MetadataResponse
			dashboard.Metadata(context.Background(), resource.MetadataRequest{ProviderTypeName: "trader"}, &response)
			got.TypeName = response.TypeName
		}
	}
	want := output{Registered: true, TypeName: "trader_dashboard"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("provider resource registration mismatch: got=%#v want=%#v", got, want)
	}
}

func dashboardTestSchema(t *testing.T) schema.Schema {
	t.Helper()
	var response resource.SchemaResponse
	(&dashboardResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response.Schema
}

func dashboardTestModel(key, config string) dashboardModel {
	return dashboardModel{Key: types.StringValue(key), Config: types.StringValue(config)}
}

func dashboardTestState(t *testing.T, model dashboardModel) tfsdk.State {
	t.Helper()
	state := tfsdk.State{Schema: dashboardTestSchema(t)}
	if diagnostics := state.Set(context.Background(), model); diagnostics.HasError() {
		t.Fatalf("build dashboard state: %v", diagnostics)
	}
	return state
}

func dashboardResponseJSON(t *testing.T, key, config string) string {
	t.Helper()
	response, err := json.Marshal(traderapigen.Dashboard{
		Config:    config,
		CreatedAt: time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		Key:       key,
		UpdatedAt: time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
	})
	if err != nil {
		t.Fatalf("encode dashboard response: %v", err)
	}
	return string(response)
}

func dashboardExpectedDiagnostic(detail string) []apiDiagnosticObservation {
	return []apiDiagnosticObservation{{
		Severity: diag.SeverityError.String(),
		Summary:  "Invalid dashboard key",
		Detail:   detail,
		Path:     path.Root("key").String(),
	}}
}
