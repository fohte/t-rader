package provider

import (
	"context"
	"io"
	"math/big"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/attr"
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

const (
	testTriggerID       = "00000000-0000-4000-8000-000000000002"
	testTriggerResponse = `{"trigger_id":"00000000-0000-4000-8000-000000000002","strategy_id":"00000000-0000-4000-8000-000000000001","purpose":"synthetic-purpose","kind":"cron","schedule":"0 9 * * 1-5","hook_slug":null,"event_match":{"status":"ready"},"prompt_template":"synthetic prompt","enabled":true,"business_days_only":true,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z"}`
)

type triggerTestRequest struct {
	Method string
	Path   string
	Body   string
}

type triggerTestDiagnostic struct {
	Severity diag.Severity
	Summary  string
	Detail   string
}

type triggerValidationOutput struct {
	Kind   []triggerTestDiagnostic
	Config []triggerTestDiagnostic
}

func TestStrategyTriggerResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	createResponse := `{"trigger_id":"00000000-0000-4000-8000-000000000002","strategy_id":"00000000-0000-4000-8000-000000000001","purpose":"synthetic-purpose","kind":"hook","schedule":null,"hook_slug":"synthetic-hook","event_match":{"attempts":9,"items":["alpha","beta"],"status":"ready"},"prompt_template":"synthetic prompt","enabled":true,"business_days_only":false,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z"}`
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusCreated, createResponse)
	})
	resourceSchema := strategyTriggerTestSchema(t)
	items, itemDiagnostics := types.TupleValue(
		[]attr.Type{types.StringType, types.StringType},
		[]attr.Value{types.StringValue("alpha"), types.StringValue("beta")},
	)
	if itemDiagnostics.HasError() {
		t.Fatalf("build event_match items: %v", itemDiagnostics)
	}
	matchAttributes := map[string]attr.Type{
		"attempts": types.NumberType,
		"items":    types.TupleType{ElemTypes: []attr.Type{types.StringType, types.StringType}},
		"status":   types.StringType,
	}
	matchValues := map[string]attr.Value{
		"attempts": triggerTestNumber(t, "9"),
		"items":    items,
		"status":   types.StringValue("ready"),
	}
	eventMatch := triggerTestObject(t, matchAttributes, matchValues)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyTriggerModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringValue("synthetic-purpose"),
		Kind:             types.StringValue("hook"),
		Schedule:         types.StringNull(),
		HookSlug:         types.StringValue("synthetic-hook"),
		EventMatch:       eventMatch,
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolValue(true),
		BusinessDaysOnly: types.BoolValue(false),
		CreatedAt:        types.StringUnknown(),
		UpdatedAt:        types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema}}
	(&strategyTriggerResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state strategyTriggerModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	type output struct {
		Request     triggerTestRequest
		State       strategyTriggerModel
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, State: state, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request: triggerTestRequest{
			Method: http.MethodPost,
			Path:   "/api/strategies/" + testStrategyID + "/triggers",
			Body:   `{"business_days_only":false,"enabled":true,"event_match":{"attempts":9,"items":["alpha","beta"],"status":"ready"},"hook_slug":"synthetic-hook","kind":"hook","prompt_template":"synthetic prompt","purpose":"synthetic-purpose"}`,
		},
		State: strategyTriggerModel{
			ID:               types.StringValue(testTriggerID),
			StrategyID:       types.StringValue(testStrategyID),
			Purpose:          types.StringValue("synthetic-purpose"),
			Kind:             types.StringValue("hook"),
			Schedule:         types.StringNull(),
			HookSlug:         types.StringValue("synthetic-hook"),
			EventMatch:       eventMatch,
			PromptTemplate:   types.StringValue("synthetic prompt"),
			Enabled:          types.BoolValue(true),
			BusinessDaysOnly: types.BoolValue(false),
			CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
			UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
		},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceCreateBusinessDaysOnly(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	createResponse := `{"trigger_id":"00000000-0000-4000-8000-000000000002","strategy_id":"00000000-0000-4000-8000-000000000001","purpose":null,"kind":"cron","schedule":"0 9 * * 1-5","hook_slug":null,"event_match":null,"prompt_template":"synthetic prompt","enabled":true,"business_days_only":true,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z"}`
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusCreated, createResponse)
	})
	resourceSchema := strategyTriggerTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyTriggerModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringNull(),
		Kind:             types.StringValue("cron"),
		Schedule:         types.StringValue("0 9 * * 1-5"),
		HookSlug:         types.StringNull(),
		EventMatch:       types.DynamicNull(),
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolNull(),
		BusinessDaysOnly: types.BoolValue(true),
		CreatedAt:        types.StringUnknown(),
		UpdatedAt:        types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema}}
	(&strategyTriggerResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state strategyTriggerModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	type output struct {
		Request     triggerTestRequest
		State       strategyTriggerModel
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, State: state, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request: triggerTestRequest{
			Method: http.MethodPost,
			Path:   "/api/strategies/" + testStrategyID + "/triggers",
			Body:   `{"business_days_only":true,"kind":"cron","prompt_template":"synthetic prompt","schedule":"0 9 * * 1-5"}`,
		},
		State: strategyTriggerModel{
			ID:               types.StringValue(testTriggerID),
			StrategyID:       types.StringValue(testStrategyID),
			Purpose:          types.StringNull(),
			Kind:             types.StringValue("cron"),
			Schedule:         types.StringValue("0 9 * * 1-5"),
			HookSlug:         types.StringNull(),
			EventMatch:       types.DynamicNull(),
			PromptTemplate:   types.StringValue("synthetic prompt"),
			Enabled:          types.BoolValue(true),
			BusinessDaysOnly: types.BoolValue(true),
			CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
			UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
		},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("business day create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusOK, testTriggerResponse)
	})
	resourceSchema := strategyTriggerTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, triggerTestStateModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	if diagnostics := state.SetAttribute(ctx, path.Root("id"), types.StringValue(testTriggerID)); diagnostics.HasError() {
		t.Fatalf("set read id: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&strategyTriggerResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var result strategyTriggerModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)
	eventMatch := triggerTestObject(t, map[string]attr.Type{"status": types.StringType}, map[string]attr.Value{"status": types.StringValue("ready")})

	type output struct {
		Request     triggerTestRequest
		State       strategyTriggerModel
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, State: result, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request: triggerTestRequest{Method: http.MethodGet, Path: "/api/triggers/" + testTriggerID},
		State: strategyTriggerModel{
			ID:               types.StringValue(testTriggerID),
			StrategyID:       types.StringValue(testStrategyID),
			Purpose:          types.StringValue("synthetic-purpose"),
			Kind:             types.StringValue("cron"),
			Schedule:         types.StringValue("0 9 * * 1-5"),
			HookSlug:         types.StringNull(),
			EventMatch:       eventMatch,
			PromptTemplate:   types.StringValue("synthetic prompt"),
			Enabled:          types.BoolValue(true),
			BusinessDaysOnly: types.BoolValue(true),
			CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
			UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
		},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceUpdateClearsEventMatch(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	updatedResponse := `{"trigger_id":"00000000-0000-4000-8000-000000000002","strategy_id":"00000000-0000-4000-8000-000000000001","purpose":null,"kind":"hook","schedule":null,"hook_slug":"synthetic-hook","event_match":null,"prompt_template":"updated synthetic prompt","enabled":false,"business_days_only":false,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-03T00:00:00Z"}`
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusOK, updatedResponse)
	})
	resourceSchema := strategyTriggerTestSchema(t)
	priorState := tfsdk.State{Schema: resourceSchema}
	priorEventMatch := triggerTestObject(t, map[string]attr.Type{"status": types.StringType}, map[string]attr.Value{"status": types.StringValue("ready")})
	if diagnostics := priorState.Set(ctx, strategyTriggerModel{
		ID:               types.StringValue(testTriggerID),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringValue("synthetic-purpose"),
		Kind:             types.StringValue("hook"),
		Schedule:         types.StringNull(),
		HookSlug:         types.StringValue("synthetic-hook"),
		EventMatch:       priorEventMatch,
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolValue(true),
		BusinessDaysOnly: types.BoolValue(false),
		CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
	}); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyTriggerModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringNull(),
		Kind:             types.StringValue("hook"),
		Schedule:         types.StringNull(),
		HookSlug:         types.StringValue("synthetic-hook"),
		EventMatch:       types.DynamicNull(),
		PromptTemplate:   types.StringValue("updated synthetic prompt"),
		Enabled:          types.BoolValue(false),
		BusinessDaysOnly: types.BoolNull(),
		CreatedAt:        types.StringUnknown(),
		UpdatedAt:        types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema}}
	(&strategyTriggerResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: priorState}, &response)
	var result strategyTriggerModel
	response.Diagnostics.Append(response.State.Get(ctx, &result)...)

	type output struct {
		Request     triggerTestRequest
		State       strategyTriggerModel
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, State: result, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request: triggerTestRequest{
			Method: http.MethodPut,
			Path:   "/api/triggers/" + testTriggerID,
			Body:   `{"enabled":false,"event_match":null,"hook_slug":"synthetic-hook","prompt_template":"updated synthetic prompt","purpose":null,"schedule":null}`,
		},
		State: strategyTriggerModel{
			ID:               types.StringValue(testTriggerID),
			StrategyID:       types.StringValue(testStrategyID),
			Purpose:          types.StringNull(),
			Kind:             types.StringValue("hook"),
			Schedule:         types.StringNull(),
			HookSlug:         types.StringValue("synthetic-hook"),
			EventMatch:       types.DynamicNull(),
			PromptTemplate:   types.StringValue("updated synthetic prompt"),
			Enabled:          types.BoolValue(false),
			BusinessDaysOnly: types.BoolValue(false),
			CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
			UpdatedAt:        types.StringValue("2026-01-03T00:00:00Z"),
		},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceUpdateEnablesBusinessDaysOnly(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	updatedResponse := `{"trigger_id":"00000000-0000-4000-8000-000000000002","strategy_id":"00000000-0000-4000-8000-000000000001","purpose":null,"kind":"cron","schedule":"0 9 * * 1-5","hook_slug":null,"event_match":null,"prompt_template":"updated synthetic prompt","enabled":true,"business_days_only":true,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-03T00:00:00Z"}`
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusOK, updatedResponse)
	})
	resourceSchema := strategyTriggerTestSchema(t)
	priorState := tfsdk.State{Schema: resourceSchema}
	if diagnostics := priorState.Set(ctx, strategyTriggerModel{
		ID:               types.StringValue(testTriggerID),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringNull(),
		Kind:             types.StringValue("cron"),
		Schedule:         types.StringValue("0 9 * * 1-5"),
		HookSlug:         types.StringNull(),
		EventMatch:       types.DynamicNull(),
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolValue(true),
		BusinessDaysOnly: types.BoolValue(false),
		CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
	}); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyTriggerModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringNull(),
		Kind:             types.StringValue("cron"),
		Schedule:         types.StringValue("0 9 * * 1-5"),
		HookSlug:         types.StringNull(),
		EventMatch:       types.DynamicNull(),
		PromptTemplate:   types.StringValue("updated synthetic prompt"),
		Enabled:          types.BoolValue(true),
		BusinessDaysOnly: types.BoolValue(true),
		CreatedAt:        types.StringUnknown(),
		UpdatedAt:        types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}
	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema}}
	(&strategyTriggerResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: priorState}, &response)
	var state strategyTriggerModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	type output struct {
		Request     triggerTestRequest
		State       strategyTriggerModel
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, State: state, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request: triggerTestRequest{
			Method: http.MethodPut,
			Path:   "/api/triggers/" + testTriggerID,
			Body:   `{"business_days_only":true,"enabled":true,"event_match":null,"hook_slug":null,"prompt_template":"updated synthetic prompt","purpose":null,"schedule":"0 9 * * 1-5"}`,
		},
		State: strategyTriggerModel{
			ID:               types.StringValue(testTriggerID),
			StrategyID:       types.StringValue(testStrategyID),
			Purpose:          types.StringNull(),
			Kind:             types.StringValue("cron"),
			Schedule:         types.StringValue("0 9 * * 1-5"),
			HookSlug:         types.StringNull(),
			EventMatch:       types.DynamicNull(),
			PromptTemplate:   types.StringValue("updated synthetic prompt"),
			Enabled:          types.BoolValue(true),
			BusinessDaysOnly: types.BoolValue(true),
			CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
			UpdatedAt:        types.StringValue("2026-01-03T00:00:00Z"),
		},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("business day update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	var observed triggerTestRequest
	client := newStrategyTriggerTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		observed = readTriggerTestRequest(t, r)
		writeTriggerTestResponse(t, w, http.StatusNoContent, "")
	})
	resourceSchema := strategyTriggerTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, triggerTestStateModel()); diagnostics.HasError() {
		t.Fatalf("build delete state: %v", diagnostics)
	}
	if diagnostics := state.SetAttribute(ctx, path.Root("id"), types.StringValue(testTriggerID)); diagnostics.HasError() {
		t.Fatalf("set delete id: %v", diagnostics)
	}
	response := resource.DeleteResponse{}
	(&strategyTriggerResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)

	type output struct {
		Request     triggerTestRequest
		Diagnostics []triggerTestDiagnostic
	}
	got := output{Request: observed, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{
		Request:     triggerTestRequest{Method: http.MethodDelete, Path: "/api/triggers/" + testTriggerID},
		Diagnostics: []triggerTestDiagnostic{},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := strategyTriggerTestSchema(t)
	response := resource.ImportStateResponse{State: tfsdk.State{Schema: resourceSchema}}
	initialState := triggerTestStateModel()
	initialState.ID = types.StringNull()
	if diagnostics := response.State.Set(ctx, initialState); diagnostics.HasError() {
		t.Fatalf("initialize import state: %v", diagnostics)
	}
	(&strategyTriggerResource{}).ImportState(ctx, resource.ImportStateRequest{ID: testTriggerID}, &response)
	var importedID types.String
	response.Diagnostics.Append(response.State.GetAttribute(ctx, path.Root("id"), &importedID)...)

	type output struct {
		ID          types.String
		Diagnostics []triggerTestDiagnostic
	}
	got := output{ID: importedID, Diagnostics: triggerTestDiagnostics(response.Diagnostics)}
	want := output{ID: types.StringValue(testTriggerID), Diagnostics: []triggerTestDiagnostic{}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestStrategyTriggerPlanValidation(t *testing.T) {
	t.Parallel()

	attributeError := func(summary, detail string) []triggerTestDiagnostic {
		return []triggerTestDiagnostic{{Severity: diag.SeverityError, Summary: summary, Detail: detail}}
	}
	cases := []struct {
		name       string
		kind       string
		schedule   types.String
		hookSlug   types.String
		eventMatch types.Dynamic
		want       triggerValidationOutput
	}{
		{
			name:       "cron requires only schedule",
			kind:       "cron",
			schedule:   types.StringValue("0 0 * * *"),
			hookSlug:   types.StringNull(),
			eventMatch: types.DynamicNull(),
			want:       triggerValidationOutput{Kind: []triggerTestDiagnostic{}, Config: []triggerTestDiagnostic{}},
		},
		{
			name:       "cron without schedule fails",
			kind:       "cron",
			schedule:   types.StringNull(),
			hookSlug:   types.StringNull(),
			eventMatch: types.DynamicNull(),
			want: triggerValidationOutput{
				Kind:   []triggerTestDiagnostic{},
				Config: attributeError("Missing schedule", "kind が cron の場合は schedule を指定してください。"),
			},
		},
		{
			name:       "cron with hook slug fails",
			kind:       "cron",
			schedule:   types.StringValue("0 0 * * *"),
			hookSlug:   types.StringValue("synthetic-hook"),
			eventMatch: types.DynamicNull(),
			want: triggerValidationOutput{
				Kind:   []triggerTestDiagnostic{},
				Config: attributeError("Unexpected hook_slug", "kind が cron の場合は hook_slug を省略してください。"),
			},
		},
		{
			name:       "hook requires only hook slug",
			kind:       "hook",
			schedule:   types.StringNull(),
			hookSlug:   types.StringValue("synthetic-hook"),
			eventMatch: types.DynamicNull(),
			want:       triggerValidationOutput{Kind: []triggerTestDiagnostic{}, Config: []triggerTestDiagnostic{}},
		},
		{
			name:       "hook without hook slug fails",
			kind:       "hook",
			schedule:   types.StringNull(),
			hookSlug:   types.StringNull(),
			eventMatch: types.DynamicNull(),
			want: triggerValidationOutput{
				Kind:   []triggerTestDiagnostic{},
				Config: attributeError("Missing hook_slug", "kind が hook の場合は hook_slug を指定してください。"),
			},
		},
		{
			name:       "hook with schedule fails",
			kind:       "hook",
			schedule:   types.StringValue("0 0 * * *"),
			hookSlug:   types.StringValue("synthetic-hook"),
			eventMatch: types.DynamicNull(),
			want: triggerValidationOutput{
				Kind:   []triggerTestDiagnostic{},
				Config: attributeError("Unexpected schedule", "kind が hook の場合は schedule を省略してください。"),
			},
		},
		{
			name:       "unsupported kind fails",
			kind:       "other",
			schedule:   types.StringNull(),
			hookSlug:   types.StringNull(),
			eventMatch: types.DynamicNull(),
			want: triggerValidationOutput{
				Kind:   attributeError("Invalid trigger kind", "kind は cron または hook にしてください。"),
				Config: []triggerTestDiagnostic{},
			},
		},
		{
			name:       "event match object is accepted",
			kind:       "hook",
			schedule:   types.StringNull(),
			hookSlug:   types.StringValue("synthetic-hook"),
			eventMatch: triggerTestObject(t, map[string]attr.Type{"status": types.StringType}, map[string]attr.Value{"status": types.StringValue("ready")}),
			want:       triggerValidationOutput{Kind: []triggerTestDiagnostic{}, Config: []triggerTestDiagnostic{}},
		},
		{
			name:       "event match must be an object",
			kind:       "hook",
			schedule:   types.StringNull(),
			hookSlug:   types.StringValue("synthetic-hook"),
			eventMatch: types.DynamicValue(types.StringValue("synthetic value")),
			want: triggerValidationOutput{
				Kind:   []triggerTestDiagnostic{},
				Config: attributeError("Invalid event_match", "event_match には object を指定してください。"),
			},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			got := validateStrategyTriggerConfig(t, testCase.kind, testCase.schedule, testCase.hookSlug, testCase.eventMatch)
			if !reflect.DeepEqual(got, testCase.want) {
				t.Fatalf("validation output mismatch: got=%#v want=%#v", got, testCase.want)
			}
		})
	}
}

func TestStrategyTriggerKindAndStrategyIDRequireReplacement(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name      string
		attribute string
		oldValue  string
		newValue  string
	}{
		{name: "kind requires replacement", attribute: "kind", oldValue: "cron", newValue: "hook"},
		{name: "strategy id requires replacement", attribute: "strategy_id", oldValue: testStrategyID, newValue: "00000000-0000-4000-8000-000000000003"},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			resourceSchema := strategyTriggerTestSchema(t)
			prior := triggerTestModelWithReplacementValue(testCase.attribute, testCase.oldValue)
			planned := triggerTestModelWithReplacementValue(testCase.attribute, testCase.newValue)
			state := tfsdk.State{Schema: resourceSchema}
			if diagnostics := state.Set(context.Background(), prior); diagnostics.HasError() {
				t.Fatalf("build prior state: %v", diagnostics)
			}
			plan := tfsdk.Plan{Schema: resourceSchema}
			if diagnostics := plan.Set(context.Background(), planned); diagnostics.HasError() {
				t.Fatalf("build plan: %v", diagnostics)
			}
			attribute := resourceSchema.Attributes[testCase.attribute].(schema.StringAttribute)
			var requiresReplace bool
			var diagnostics diag.Diagnostics
			for _, modifier := range attribute.PlanModifiers {
				var response planmodifier.StringResponse
				modifier.PlanModifyString(context.Background(), planmodifier.StringRequest{
					Path:        path.Root(testCase.attribute),
					State:       state,
					Plan:        plan,
					StateValue:  types.StringValue(testCase.oldValue),
					PlanValue:   types.StringValue(testCase.newValue),
					ConfigValue: types.StringValue(testCase.newValue),
				}, &response)
				requiresReplace = requiresReplace || response.RequiresReplace
				diagnostics.Append(response.Diagnostics...)
			}
			type output struct {
				RequiresReplace bool
				Diagnostics     []triggerTestDiagnostic
			}
			got := output{RequiresReplace: requiresReplace, Diagnostics: triggerTestDiagnostics(diagnostics)}
			want := output{RequiresReplace: true, Diagnostics: []triggerTestDiagnostic{}}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("replacement output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func validateStrategyTriggerConfig(t *testing.T, kind string, schedule, hookSlug types.String, eventMatch types.Dynamic) triggerValidationOutput {
	return validateStrategyTriggerConfigWithBusinessDaysOnly(t, kind, schedule, hookSlug, eventMatch, types.BoolNull())
}

func validateStrategyTriggerConfigWithBusinessDaysOnly(
	t *testing.T,
	kind string,
	schedule, hookSlug types.String,
	eventMatch types.Dynamic,
	businessDaysOnly types.Bool,
) triggerValidationOutput {
	t.Helper()
	ctx := context.Background()
	resourceSchema := strategyTriggerTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, strategyTriggerModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringNull(),
		Kind:             types.StringValue(kind),
		Schedule:         schedule,
		HookSlug:         hookSlug,
		EventMatch:       eventMatch,
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolUnknown(),
		BusinessDaysOnly: businessDaysOnly,
		CreatedAt:        types.StringUnknown(),
		UpdatedAt:        types.StringUnknown(),
	}); diagnostics.HasError() {
		t.Fatalf("build validation plan: %v", diagnostics)
	}
	var kindResponse validator.StringResponse
	kindAttribute := resourceSchema.Attributes["kind"].(schema.StringAttribute)
	for _, stringValidator := range kindAttribute.Validators {
		stringValidator.ValidateString(ctx, validator.StringRequest{Path: path.Root("kind"), ConfigValue: types.StringValue(kind)}, &kindResponse)
	}
	var configDiagnostics diag.Diagnostics
	request := resource.ValidateConfigRequest{
		Config: tfsdk.Config{Raw: plan.Raw, Schema: resourceSchema},
	}
	for _, configValidator := range (&strategyTriggerResource{}).ConfigValidators(ctx) {
		var response resource.ValidateConfigResponse
		configValidator.ValidateResource(ctx, request, &response)
		configDiagnostics.Append(response.Diagnostics...)
	}
	return triggerValidationOutput{Kind: triggerTestDiagnostics(kindResponse.Diagnostics), Config: triggerTestDiagnostics(configDiagnostics)}
}

func TestStrategyTriggerRejectsBusinessDaysOnlyForHook(t *testing.T) {
	t.Parallel()

	got := validateStrategyTriggerConfigWithBusinessDaysOnly(
		t,
		"hook",
		types.StringNull(),
		types.StringValue("synthetic-hook"),
		types.DynamicNull(),
		types.BoolValue(true),
	)
	want := triggerValidationOutput{
		Kind: []triggerTestDiagnostic{},
		Config: []triggerTestDiagnostic{{
			Severity: diag.SeverityError,
			Summary:  "Unexpected business_days_only",
			Detail:   "business_days_only は kind が cron の場合にのみ true にできます。",
		}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("validation output mismatch: got=%#v want=%#v", got, want)
	}
}

func triggerTestStateModel() strategyTriggerModel {
	return strategyTriggerModel{
		ID:               types.StringValue(testTriggerID),
		StrategyID:       types.StringValue(testStrategyID),
		Purpose:          types.StringValue("synthetic-purpose"),
		Kind:             types.StringValue("hook"),
		Schedule:         types.StringNull(),
		HookSlug:         types.StringValue("synthetic-hook"),
		EventMatch:       types.DynamicNull(),
		PromptTemplate:   types.StringValue("synthetic prompt"),
		Enabled:          types.BoolValue(true),
		BusinessDaysOnly: types.BoolValue(false),
		CreatedAt:        types.StringValue("2026-01-01T00:00:00Z"),
		UpdatedAt:        types.StringValue("2026-01-02T00:00:00Z"),
	}
}

func triggerTestModelWithReplacementValue(attribute, value string) strategyTriggerModel {
	model := triggerTestStateModel()
	if attribute == "kind" {
		model.Kind = types.StringValue(value)
	} else {
		model.StrategyID = types.StringValue(value)
	}
	return model
}

func strategyTriggerTestSchema(t *testing.T) schema.Schema {
	t.Helper()
	var response resource.SchemaResponse
	(&strategyTriggerResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response.Schema
}

func newStrategyTriggerTestClient(t *testing.T, handler http.HandlerFunc) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create trader API client: %v", err)
	}
	return client
}

func readTriggerTestRequest(t *testing.T, request *http.Request) triggerTestRequest {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
	}
	return triggerTestRequest{Method: request.Method, Path: request.URL.Path, Body: string(body)}
}

func writeTriggerTestResponse(t *testing.T, writer http.ResponseWriter, status int, body string) {
	t.Helper()
	if body != "" {
		writer.Header().Set("Content-Type", "application/json")
	}
	writer.WriteHeader(status)
	if _, err := io.WriteString(writer, body); err != nil {
		t.Errorf("write response body: %v", err)
	}
}

func triggerTestObject(t *testing.T, typesByName map[string]attr.Type, values map[string]attr.Value) types.Dynamic {
	t.Helper()
	value, diagnostics := types.ObjectValue(typesByName, values)
	if diagnostics.HasError() {
		t.Fatalf("build event_match object: %v", diagnostics)
	}
	return types.DynamicValue(value)
}

func triggerTestNumber(t *testing.T, value string) types.Number {
	t.Helper()
	number, _, err := big.ParseFloat(value, 10, 256, big.ToNearestEven)
	if err != nil {
		t.Fatalf("parse test number: %v", err)
	}
	return types.NumberValue(number)
}

func triggerTestDiagnostics(diagnostics diag.Diagnostics) []triggerTestDiagnostic {
	result := make([]triggerTestDiagnostic, 0, len(diagnostics))
	for _, diagnostic := range diagnostics {
		result = append(result, triggerTestDiagnostic{
			Severity: diagnostic.Severity(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		})
	}
	return result
}
