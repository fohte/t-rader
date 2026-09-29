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
	"github.com/hashicorp/terraform-plugin-go/tftypes"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

const (
	testRssFeedID      = "00000000-0000-4000-8000-000000000101"
	testOtherRssFeedID = "00000000-0000-4000-8000-000000000102"
)

type rssFeedRequestObservation struct {
	Method string
	Path   string
	Body   string
}

type rssFeedDiagnosticObservation struct {
	Severity string
	Summary  string
	Detail   string
	Path     string
}

func TestRssFeedResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	feed := syntheticRssFeed(testRssFeedID, "synthetic_feed", "Synthetic Feed", "https://feeds.example.invalid/synthetic.xml", true)
	requests := make(chan rssFeedRequestObservation, 1)
	client := newRssFeedTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordRssFeedRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		if err := json.NewEncoder(w).Encode(feed); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := rssFeedResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, rssFeedModel{
		ID:          types.StringUnknown(),
		Source:      types.StringValue("synthetic_feed"),
		DisplayName: types.StringValue("Synthetic Feed"),
		URL:         types.StringValue("https://feeds.example.invalid/synthetic.xml"),
		Enabled:     types.BoolUnknown(),
		CreatedAt:   types.StringUnknown(),
		UpdatedAt:   types.StringUnknown(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build create plan: %v", planDiagnostics)
	}

	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	(&rssFeedResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var observedRequest *rssFeedRequestObservation
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}
	var resultState rssFeedModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	type output struct {
		Request     *rssFeedRequestObservation
		State       rssFeedModel
		Diagnostics []rssFeedDiagnosticObservation
	}
	got := output{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: rssFeedDiagnosticsOutput(response.Diagnostics),
	}
	want := output{
		Request: &rssFeedRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/rss-feeds",
			Body:   `{"display_name":"Synthetic Feed","source":"synthetic_feed","url":"https://feeds.example.invalid/synthetic.xml"}`,
		},
		State:       modelFromRssFeed(feed),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	feed := syntheticRssFeed(testRssFeedID, "synthetic_feed", "Refreshed Feed", "https://feeds.example.invalid/refreshed.xml", true)
	requests := make(chan rssFeedRequestObservation, 1)
	client := newRssFeedTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordRssFeedRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		if err := json.NewEncoder(w).Encode(feed); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := rssFeedResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, rssFeedModel{
		ID:          types.StringValue(testRssFeedID),
		Source:      types.StringValue("synthetic_feed"),
		DisplayName: types.StringValue("Stale Feed"),
		URL:         types.StringValue("https://feeds.example.invalid/stale.xml"),
		Enabled:     types.BoolValue(false),
		CreatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:   types.StringValue("2026-01-01T00:00:00Z"),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}

	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}
	(&rssFeedResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var observedRequest *rssFeedRequestObservation
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}
	var resultState rssFeedModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	type output struct {
		Request     *rssFeedRequestObservation
		State       rssFeedModel
		Diagnostics []rssFeedDiagnosticObservation
	}
	got := output{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: rssFeedDiagnosticsOutput(response.Diagnostics),
	}
	want := output{
		Request: &rssFeedRequestObservation{
			Method: http.MethodGet,
			Path:   "/api/rss-feeds/" + testRssFeedID,
		},
		State:       modelFromRssFeed(feed),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceReadRemovesMissingFeed(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan rssFeedRequestObservation, 1)
	client := newRssFeedTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordRssFeedRequest(t, requests, r) {
			return
		}
		http.Error(w, "synthetic missing feed", http.StatusNotFound)
	})
	resourceSchema := rssFeedResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticRssFeedModel(testRssFeedID))
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}

	response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}
	(&rssFeedResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var observedRequest *rssFeedRequestObservation
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}
	got := struct {
		Request      *rssFeedRequestObservation
		StateRemoved bool
		Diagnostics  []rssFeedDiagnosticObservation
	}{
		Request:      observedRequest,
		StateRemoved: response.State.Raw.IsNull(),
		Diagnostics:  rssFeedDiagnosticsOutput(response.Diagnostics),
	}
	want := struct {
		Request      *rssFeedRequestObservation
		StateRemoved bool
		Diagnostics  []rssFeedDiagnosticObservation
	}{
		Request: &rssFeedRequestObservation{
			Method: http.MethodGet,
			Path:   "/api/rss-feeds/" + testRssFeedID,
		},
		StateRemoved: true,
		Diagnostics:  nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read missing output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceUpdateUsesPriorStateID(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	feed := syntheticRssFeed(testRssFeedID, "synthetic_feed", "Updated Feed", "https://feeds.example.invalid/updated.xml", false)
	requests := make(chan rssFeedRequestObservation, 1)
	client := newRssFeedTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordRssFeedRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		if err := json.NewEncoder(w).Encode(feed); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := rssFeedResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticRssFeedModel(testRssFeedID))
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, rssFeedModel{
		ID:          types.StringValue(testOtherRssFeedID),
		Source:      types.StringValue("synthetic_feed"),
		DisplayName: types.StringValue("Updated Feed"),
		URL:         types.StringValue("https://feeds.example.invalid/updated.xml"),
		Enabled:     types.BoolValue(false),
		CreatedAt:   types.StringUnknown(),
		UpdatedAt:   types.StringUnknown(),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&rssFeedResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	var observedRequest *rssFeedRequestObservation
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}
	var resultState rssFeedModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	type output struct {
		Request     *rssFeedRequestObservation
		State       rssFeedModel
		Diagnostics []rssFeedDiagnosticObservation
	}
	got := output{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: rssFeedDiagnosticsOutput(response.Diagnostics),
	}
	want := output{
		Request: &rssFeedRequestObservation{
			Method: http.MethodPatch,
			Path:   "/api/rss-feeds/" + testRssFeedID,
			Body:   `{"display_name":"Updated Feed","enabled":false,"url":"https://feeds.example.invalid/updated.xml"}`,
		},
		State:       modelFromRssFeed(feed),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan rssFeedRequestObservation, 1)
	client := newRssFeedTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordRssFeedRequest(t, requests, r) {
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	resourceSchema := rssFeedResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticRssFeedModel(testRssFeedID))
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}

	response := resource.DeleteResponse{State: state}
	(&rssFeedResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	var observedRequest *rssFeedRequestObservation
	select {
	case observed := <-requests:
		observedRequest = &observed
	default:
	}
	got := struct {
		Request     *rssFeedRequestObservation
		Diagnostics []rssFeedDiagnosticObservation
	}{Request: observedRequest, Diagnostics: rssFeedDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *rssFeedRequestObservation
		Diagnostics []rssFeedDiagnosticObservation
	}{
		Request: &rssFeedRequestObservation{
			Method: http.MethodDelete,
			Path:   "/api/rss-feeds/" + testRssFeedID,
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := rssFeedResourceSchema(t)
	response := resource.ImportStateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	stateDiagnostics := response.State.Set(ctx, rssFeedModel{
		ID:          types.StringNull(),
		Source:      types.StringNull(),
		DisplayName: types.StringNull(),
		URL:         types.StringNull(),
		Enabled:     types.BoolNull(),
		CreatedAt:   types.StringNull(),
		UpdatedAt:   types.StringNull(),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build import state: %v", stateDiagnostics)
	}
	(&rssFeedResource{}).ImportState(ctx, resource.ImportStateRequest{ID: testRssFeedID}, &response)
	var imported rssFeedModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)

	got := struct {
		State       rssFeedModel
		Diagnostics []rssFeedDiagnosticObservation
	}{State: imported, Diagnostics: rssFeedDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       rssFeedModel
		Diagnostics []rssFeedDiagnosticObservation
	}{
		State: rssFeedModel{
			ID:          types.StringValue(testRssFeedID),
			Source:      types.StringNull(),
			DisplayName: types.StringNull(),
			URL:         types.StringNull(),
			Enabled:     types.BoolNull(),
			CreatedAt:   types.StringNull(),
			UpdatedAt:   types.StringNull(),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRssFeedResourceValidators(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name      string
		attribute string
		value     types.String
		want      []rssFeedDiagnosticObservation
	}{
		{name: "source null is deferred", attribute: "source", value: types.StringNull()},
		{name: "source unknown is deferred", attribute: "source", value: types.StringUnknown()},
		{name: "source accepts lowercase key", attribute: "source", value: types.StringValue("synthetic_feed-2")},
		{
			name:      "source rejects empty key",
			attribute: "source",
			value:     types.StringValue(""),
			want:      rssFeedExpectedDiagnostic(path.Root("source"), "Invalid RSS feed source", "Source must contain only lowercase letters, digits, hyphens, and underscores without surrounding whitespace."),
		},
		{
			name:      "source rejects uppercase key",
			attribute: "source",
			value:     types.StringValue("Synthetic_Feed"),
			want:      rssFeedExpectedDiagnostic(path.Root("source"), "Invalid RSS feed source", "Source must contain only lowercase letters, digits, hyphens, and underscores without surrounding whitespace."),
		},
		{
			name:      "source rejects surrounding whitespace",
			attribute: "source",
			value:     types.StringValue(" synthetic_feed "),
			want:      rssFeedExpectedDiagnostic(path.Root("source"), "Invalid RSS feed source", "Source must contain only lowercase letters, digits, hyphens, and underscores without surrounding whitespace."),
		},
		{name: "display name null is deferred", attribute: "display_name", value: types.StringNull()},
		{name: "display name unknown is deferred", attribute: "display_name", value: types.StringUnknown()},
		{name: "display name accepts ordinary text", attribute: "display_name", value: types.StringValue("Synthetic Feed")},
		{
			name:      "display name rejects whitespace only",
			attribute: "display_name",
			value:     types.StringValue(" \t "),
			want:      rssFeedExpectedDiagnostic(path.Root("display_name"), "Invalid RSS feed display name", "Display name must not be empty or have surrounding whitespace."),
		},
		{
			name:      "display name rejects surrounding whitespace",
			attribute: "display_name",
			value:     types.StringValue(" Synthetic Feed "),
			want:      rssFeedExpectedDiagnostic(path.Root("display_name"), "Invalid RSS feed display name", "Display name must not be empty or have surrounding whitespace."),
		},
		{name: "url null is deferred", attribute: "url", value: types.StringNull()},
		{name: "url unknown is deferred", attribute: "url", value: types.StringUnknown()},
		{name: "url accepts https", attribute: "url", value: types.StringValue("https://feeds.example.invalid/synthetic.xml")},
		{name: "url accepts http", attribute: "url", value: types.StringValue("http://feeds.example.invalid/synthetic.xml")},
		{
			name:      "url rejects empty value",
			attribute: "url",
			value:     types.StringValue(""),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
		{
			name:      "url rejects relative path",
			attribute: "url",
			value:     types.StringValue("/synthetic.xml"),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
		{
			name:      "url rejects missing host",
			attribute: "url",
			value:     types.StringValue("https:/synthetic.xml"),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
		{
			name:      "url rejects malformed authority",
			attribute: "url",
			value:     types.StringValue("https://["),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
		{
			name:      "url rejects unsupported scheme",
			attribute: "url",
			value:     types.StringValue("ftp://feeds.example.invalid/synthetic.xml"),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
		{
			name:      "url rejects surrounding whitespace",
			attribute: "url",
			value:     types.StringValue(" https://feeds.example.invalid/synthetic.xml"),
			want:      rssFeedExpectedDiagnostic(path.Root("url"), "Invalid RSS feed URL", "URL must be an absolute HTTP or HTTPS URL without surrounding whitespace."),
		},
	}
	resourceSchema := rssFeedResourceSchema(t)
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			attribute := resourceSchema.Schema.Attributes[testCase.attribute].(schema.StringAttribute)
			var response validator.StringResponse
			attribute.Validators[0].ValidateString(context.Background(), validator.StringRequest{
				Path:        path.Root(testCase.attribute),
				ConfigValue: testCase.value,
			}, &response)
			got := rssFeedDiagnosticsOutput(response.Diagnostics)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validator output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func TestRssFeedResourcePlanModifiers(t *testing.T) {
	t.Parallel()

	type modifierOutput struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []rssFeedDiagnosticObservation
	}
	cases := []struct {
		name              string
		attribute         string
		stateValue        types.String
		planValue         types.String
		configValue       types.String
		withoutPriorState bool
		want              modifierOutput
	}{
		{
			name:        "unchanged source does not replace",
			attribute:   "source",
			stateValue:  types.StringValue("synthetic_feed"),
			planValue:   types.StringValue("synthetic_feed"),
			configValue: types.StringValue("synthetic_feed"),
			want:        modifierOutput{PlanValue: types.StringValue("synthetic_feed")},
		},
		{
			name:        "changed source requires replacement",
			attribute:   "source",
			stateValue:  types.StringValue("synthetic_feed"),
			planValue:   types.StringValue("renamed_feed"),
			configValue: types.StringValue("renamed_feed"),
			want:        modifierOutput{PlanValue: types.StringValue("renamed_feed"), RequiresReplace: true},
		},
		{
			name:        "id uses prior state when plan is unknown",
			attribute:   "id",
			stateValue:  types.StringValue(testRssFeedID),
			planValue:   types.StringUnknown(),
			configValue: types.StringNull(),
			want:        modifierOutput{PlanValue: types.StringValue(testRssFeedID)},
		},
		{
			name:              "id stays unknown without prior state",
			attribute:         "id",
			stateValue:        types.StringNull(),
			planValue:         types.StringUnknown(),
			configValue:       types.StringNull(),
			withoutPriorState: true,
			want:              modifierOutput{PlanValue: types.StringUnknown()},
		},
	}
	resourceSchema := rssFeedResourceSchema(t)
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			attribute := resourceSchema.Schema.Attributes[testCase.attribute].(schema.StringAttribute)
			ctx := context.Background()
			state := tfsdk.State{Schema: resourceSchema.Schema}
			if testCase.withoutPriorState {
				state.Raw = tftypes.NewValue(resourceSchema.Schema.Type().TerraformType(ctx), nil)
			} else {
				stateDiagnostics := state.Set(ctx, rssFeedModelWithStringAttribute(syntheticRssFeedModel(testRssFeedID), testCase.attribute, testCase.stateValue))
				if stateDiagnostics.HasError() {
					t.Fatalf("build prior state: %v", stateDiagnostics)
				}
			}
			plan := tfsdk.Plan{Schema: resourceSchema.Schema}
			planDiagnostics := plan.Set(ctx, rssFeedModelWithStringAttribute(syntheticRssFeedModel(testRssFeedID), testCase.attribute, testCase.planValue))
			if planDiagnostics.HasError() {
				t.Fatalf("build plan: %v", planDiagnostics)
			}
			var got modifierOutput
			for _, modifier := range attribute.PlanModifiers {
				request := planmodifier.StringRequest{
					Path:        path.Root(testCase.attribute),
					State:       state,
					Plan:        plan,
					StateValue:  testCase.stateValue,
					PlanValue:   testCase.planValue,
					ConfigValue: testCase.configValue,
				}
				response := planmodifier.StringResponse{PlanValue: request.PlanValue}
				modifier.PlanModifyString(context.Background(), request, &response)
				got = modifierOutput{
					PlanValue:       response.PlanValue,
					RequiresReplace: response.RequiresReplace,
					Diagnostics:     rssFeedDiagnosticsOutput(response.Diagnostics),
				}
			}
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("plan modifier output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func TestRssFeedResourceEnabledPlanModifierUsesPriorValueWhenUnknown(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := rssFeedResourceSchema(t)
	stateModel := syntheticRssFeedModel(testRssFeedID)
	stateModel.Enabled = types.BoolValue(false)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, stateModel)
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	planModel := syntheticRssFeedModel(testRssFeedID)
	planModel.Enabled = types.BoolUnknown()
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, planModel)
	if planDiagnostics.HasError() {
		t.Fatalf("build plan: %v", planDiagnostics)
	}

	attribute := resourceSchema.Schema.Attributes["enabled"].(schema.BoolAttribute)
	var got planmodifier.BoolResponse
	for _, modifier := range attribute.PlanModifiers {
		request := planmodifier.BoolRequest{
			Path:        path.Root("enabled"),
			State:       state,
			Plan:        plan,
			StateValue:  stateModel.Enabled,
			PlanValue:   planModel.Enabled,
			ConfigValue: types.BoolNull(),
		}
		got = planmodifier.BoolResponse{PlanValue: request.PlanValue}
		modifier.PlanModifyBool(ctx, request, &got)
	}
	want := planmodifier.BoolResponse{PlanValue: types.BoolValue(false)}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("enabled plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func rssFeedModelWithStringAttribute(model rssFeedModel, attribute string, value types.String) rssFeedModel {
	switch attribute {
	case "id":
		model.ID = value
	case "source":
		model.Source = value
	case "display_name":
		model.DisplayName = value
	case "url":
		model.URL = value
	case "created_at":
		model.CreatedAt = value
	case "updated_at":
		model.UpdatedAt = value
	}
	return model
}

func newRssFeedTestClient(t *testing.T, handler http.HandlerFunc) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}
	return client
}

func recordRssFeedRequest(t *testing.T, requests chan<- rssFeedRequestObservation, request *http.Request) bool {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
		return false
	}
	requests <- rssFeedRequestObservation{Method: request.Method, Path: request.URL.Path, Body: string(body)}
	return true
}

func rssFeedResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&rssFeedResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func syntheticRssFeed(id, source, displayName, url string, enabled bool) traderapi.RssFeed {
	return traderapi.RssFeed{
		Id:          uuid.MustParse(id),
		Source:      source,
		DisplayName: displayName,
		Url:         url,
		Enabled:     enabled,
		CreatedAt:   time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		UpdatedAt:   time.Date(2026, time.January, 2, 0, 0, 0, 0, time.UTC),
	}
}

func syntheticRssFeedModel(id string) rssFeedModel {
	return modelFromRssFeed(syntheticRssFeed(id, "synthetic_feed", "Synthetic Feed", "https://feeds.example.invalid/synthetic.xml", true))
}

func rssFeedExpectedDiagnostic(diagnosticPath path.Path, summary, detail string) []rssFeedDiagnosticObservation {
	return []rssFeedDiagnosticObservation{{
		Severity: diag.SeverityError.String(),
		Summary:  summary,
		Detail:   detail,
		Path:     diagnosticPath.String(),
	}}
}

func rssFeedDiagnosticsOutput(diagnostics diag.Diagnostics) []rssFeedDiagnosticObservation {
	var result []rssFeedDiagnosticObservation
	for _, diagnostic := range diagnostics {
		observation := rssFeedDiagnosticObservation{
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
