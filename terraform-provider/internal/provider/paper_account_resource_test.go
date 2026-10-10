package provider

import (
	"context"
	"net/http"
	"reflect"
	"strings"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
)

const (
	testPaperAccountID         = "00000000-0000-4000-8000-000000000302"
	testPaperAccountStrategyID = "00000000-0000-4000-8000-000000000abc"
	testPaperAccountPurpose    = "sample"
	testPaperAccountName       = "sample-paper-account"
	testPaperAccountStartedOn  = "2026-01-02"
)

func TestPaperAccountResourceCreateUsesAPIResponseAsState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		recordAPIResourceRequest(t, requests, r)
		writeAPIResourceResponse(t, w, http.StatusCreated, `{"id":"`+testPaperAccountID+`","strategy_id":"`+testPaperAccountStrategyID+`","purpose":"sample","name":"sample-paper-account","initial_cash_jpy":1000000,"benchmark_stock_id":"synthetic-stock-id","started_on":"2026-01-02"}`)
	})
	resourceSchema := paperAccountTestSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema}
	if diagnostics := plan.Set(ctx, paperAccountModel{
		ID:               types.StringUnknown(),
		StrategyID:       types.StringValue(testPaperAccountStrategyID),
		Purpose:          types.StringValue(testPaperAccountPurpose),
		Name:             types.StringValue(testPaperAccountName),
		InitialCashJpy:   types.Float64Value(1000000),
		BenchmarkStockID: types.StringValue("synthetic-stock-id"),
		StartedOn:        types.StringValue(testPaperAccountStartedOn),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}
	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema}}
	(&paperAccountResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state paperAccountModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)

	type output struct {
		Request     apiRequestObservation
		State       paperAccountModel
		Diagnostics []apiDiagnosticObservation
	}
	got := output{Request: *receiveAPIResourceRequest(requests), State: state, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request: apiRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/paper-accounts",
			Body:   `{"benchmark_stock_id":"synthetic-stock-id","initial_cash_jpy":1000000,"name":"sample-paper-account","purpose":"sample","started_on":"2026-01-02","strategy_id":"` + testPaperAccountStrategyID + `"}`,
		},
		State: paperAccountModel{
			ID:               types.StringValue(testPaperAccountID),
			StrategyID:       types.StringValue(testPaperAccountStrategyID),
			Purpose:          types.StringValue(testPaperAccountPurpose),
			Name:             types.StringValue(testPaperAccountName),
			InitialCashJpy:   types.Float64Value(1000000),
			BenchmarkStockID: types.StringValue("synthetic-stock-id"),
			StartedOn:        types.StringValue(testPaperAccountStartedOn),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceReadRefreshesStateFromAPI(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		recordAPIResourceRequest(t, requests, r)
		writeAPIResourceResponse(t, w, http.StatusOK, `[{"id":"00000000-0000-4000-8000-000000000303","strategy_id":"`+testPaperAccountStrategyID+`","purpose":"other","name":"other-account","initial_cash_jpy":2000000,"benchmark_stock_id":null,"started_on":"2026-01-03"},{"id":"`+testPaperAccountID+`","strategy_id":"`+testPaperAccountStrategyID+`","purpose":"sample","name":"sample-paper-account","initial_cash_jpy":1000000,"benchmark_stock_id":null,"started_on":"2026-01-02"}]`)
	})
	resourceSchema := paperAccountTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, paperAccountTestModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&paperAccountResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var refreshed paperAccountModel
	response.Diagnostics.Append(response.State.Get(ctx, &refreshed)...)

	type output struct {
		Request     apiRequestObservation
		State       paperAccountModel
		Diagnostics []apiDiagnosticObservation
	}
	expectedState := paperAccountTestModel()
	expectedState.BenchmarkStockID = types.StringNull()
	got := output{Request: *receiveAPIResourceRequest(requests), State: refreshed, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request: apiRequestObservation{Method: http.MethodGet, Path: "/api/paper-accounts"},
		State:   expectedState,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceReadDropsMissingAccountFromState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		recordAPIResourceRequest(t, requests, r)
		writeAPIResourceResponse(t, w, http.StatusOK, `[]`)
	})
	resourceSchema := paperAccountTestSchema(t)
	state := tfsdk.State{Schema: resourceSchema}
	if diagnostics := state.Set(ctx, paperAccountTestModel()); diagnostics.HasError() {
		t.Fatalf("build read state: %v", diagnostics)
	}
	response := resource.ReadResponse{State: state}
	(&paperAccountResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)

	type output struct {
		Request      apiRequestObservation
		StateRemoved bool
		Diagnostics  []apiDiagnosticObservation
	}
	got := output{Request: *receiveAPIResourceRequest(requests), StateRemoved: response.State.Raw.IsNull(), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := output{
		Request:      apiRequestObservation{Method: http.MethodGet, Path: "/api/paper-accounts"},
		StateRemoved: true,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("missing account output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceUpdateRejectsAttributeChanges(t *testing.T) {
	t.Parallel()

	response := resource.UpdateResponse{}
	(&paperAccountResource{}).Update(context.Background(), resource.UpdateRequest{}, &response)
	got := apiResourceDiagnosticsOutput(response.Diagnostics)
	want := []apiDiagnosticObservation{{
		Severity: "Error",
		Summary:  "Paper account attributes are immutable",
		Detail:   "作成後の口座属性は変更できません。設定を元に戻すか、Terraform 管理から外す場合はリソースを構成から削除して `terraform state rm` を実行してください。",
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update diagnostics mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceDeleteWarnsThatAccountRemains(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	state := tfsdk.State{Schema: paperAccountTestSchema(t)}
	if diagnostics := state.Set(ctx, paperAccountTestModel()); diagnostics.HasError() {
		t.Fatalf("build delete state: %v", diagnostics)
	}
	response := resource.DeleteResponse{}
	(&paperAccountResource{}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	got := apiResourceDiagnosticsOutput(response.Diagnostics)
	want := []apiDiagnosticObservation{{
		Severity: "Warning",
		Summary:  "Paper account remains in the backend",
		Detail:   "Terraform state から口座 sample-paper-account (" + testPaperAccountID + ") を外しますが、バックエンドの口座は削除されません。",
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete diagnostics mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceImportSetsAccountID(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	state := tfsdk.State{Schema: paperAccountTestSchema(t)}
	if diagnostics := state.Set(ctx, paperAccountModel{
		ID:               types.StringNull(),
		StrategyID:       types.StringNull(),
		Purpose:          types.StringNull(),
		Name:             types.StringNull(),
		InitialCashJpy:   types.Float64Null(),
		BenchmarkStockID: types.StringNull(),
		StartedOn:        types.StringNull(),
	}); diagnostics.HasError() {
		t.Fatalf("initialize import state: %v", diagnostics)
	}
	response := resource.ImportStateResponse{State: state}
	(&paperAccountResource{}).ImportState(ctx, resource.ImportStateRequest{ID: strings.ToUpper(testPaperAccountID)}, &response)
	var imported paperAccountModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)

	got := struct {
		State       paperAccountModel
		Diagnostics []apiDiagnosticObservation
	}{State: imported, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       paperAccountModel
		Diagnostics []apiDiagnosticObservation
	}{
		State: paperAccountModel{
			ID:               types.StringValue(testPaperAccountID),
			StrategyID:       types.StringNull(),
			Purpose:          types.StringNull(),
			Name:             types.StringNull(),
			InitialCashJpy:   types.Float64Null(),
			BenchmarkStockID: types.StringNull(),
			StartedOn:        types.StringNull(),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceImportRejectsInvalidID(t *testing.T) {
	t.Parallel()

	response := resource.ImportStateResponse{State: tfsdk.State{Schema: paperAccountTestSchema(t)}}
	(&paperAccountResource{}).ImportState(context.Background(), resource.ImportStateRequest{ID: "synthetic-account-id"}, &response)
	got := apiResourceDiagnosticsOutput(response.Diagnostics)
	want := []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid Import ID", Detail: "Import ID must be a paper account UUID."}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("invalid import diagnostics mismatch: got=%#v want=%#v", got, want)
	}
}

func TestPaperAccountResourceCreateRejectsInvalidInputBeforeRequest(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name     string
		field    string
		value    string
		wantDiag apiDiagnosticObservation
	}{
		{
			name:  "invalid strategy UUID",
			field: "strategy_id",
			value: "synthetic-strategy-id",
			wantDiag: apiDiagnosticObservation{
				Severity: "Error",
				Summary:  "Invalid strategy_id",
				Detail:   "strategy_id は UUID にしてください。",
				Path:     "strategy_id",
			},
		},
		{
			name:  "non-canonical strategy UUID",
			field: "strategy_id",
			value: strings.ToUpper(testPaperAccountStrategyID),
			wantDiag: apiDiagnosticObservation{
				Severity: "Error",
				Summary:  "Non-canonical strategy_id",
				Detail:   "strategy_id は小文字の正規 UUID にしてください。",
				Path:     "strategy_id",
			},
		},
		{
			name:  "invalid start date",
			field: "started_on",
			value: "2026-02-30",
			wantDiag: apiDiagnosticObservation{
				Severity: "Error",
				Summary:  "Invalid started_on",
				Detail:   "started_on は YYYY-MM-DD 形式にしてください。",
				Path:     "started_on",
			},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			requests := make(chan apiRequestObservation, 1)
			client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				recordAPIResourceRequest(t, requests, r)
				writeAPIResourceResponse(t, w, http.StatusCreated, `{}`)
			})
			resourceSchema := paperAccountTestSchema(t)
			plan := tfsdk.Plan{Schema: resourceSchema}
			model := paperAccountTestModel()
			if testCase.field == "strategy_id" {
				model.StrategyID = types.StringValue(testCase.value)
			} else {
				model.StartedOn = types.StringValue(testCase.value)
			}
			if diagnostics := plan.Set(ctx, model); diagnostics.HasError() {
				t.Fatalf("build invalid create plan: %v", diagnostics)
			}
			response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema}}
			(&paperAccountResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)

			type output struct {
				Requests    int
				Diagnostics []apiDiagnosticObservation
			}
			got := output{Requests: len(requests), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
			want := output{Diagnostics: []apiDiagnosticObservation{testCase.wantDiag}}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("invalid create output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestPaperAccountResourceValidatorsRejectNormalizedValues(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name      string
		attribute string
		value     string
		want      []apiDiagnosticObservation
	}{
		{name: "canonical strategy UUID is accepted", attribute: "strategy_id", value: testPaperAccountStrategyID},
		{name: "uppercase strategy UUID is rejected", attribute: "strategy_id", value: strings.ToUpper(testPaperAccountStrategyID), want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Non-canonical strategy_id", Detail: "strategy_id は小文字の正規 UUID にしてください。", Path: "strategy_id"}}},
		{name: "non-empty account name is accepted", attribute: "name", value: testPaperAccountName},
		{name: "whitespace-only account name is rejected", attribute: "name", value: "   ", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid paper account name", Detail: "name は空にできません。", Path: "name"}}},
		{name: "padded account name is rejected", attribute: "name", value: " sample-account ", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid paper account name", Detail: "name の前後に空白を指定できません。", Path: "name"}}},
		{name: "non-empty purpose is accepted", attribute: "purpose", value: testPaperAccountPurpose},
		{name: "padded purpose is rejected", attribute: "purpose", value: " sample ", want: []apiDiagnosticObservation{{Severity: "Error", Summary: "Invalid paper account purpose", Detail: "purpose の前後に空白を指定できません。", Path: "purpose"}}},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()
			attribute := paperAccountTestSchema(t).Attributes[testCase.attribute].(schema.StringAttribute)
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

func TestProviderRegistersPaperAccountResource(t *testing.T) {
	t.Parallel()

	type output struct {
		Registered bool
		TypeName   string
	}
	got := output{}
	for _, factory := range (&traderProvider{}).Resources(context.Background()) {
		if accountResource, ok := factory().(*paperAccountResource); ok {
			got.Registered = true
			var response resource.MetadataResponse
			accountResource.Metadata(context.Background(), resource.MetadataRequest{ProviderTypeName: "trader"}, &response)
			got.TypeName = response.TypeName
		}
	}
	want := output{Registered: true, TypeName: "trader_paper_account"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("provider resource registration mismatch: got=%#v want=%#v", got, want)
	}
}

func paperAccountTestSchema(t *testing.T) schema.Schema {
	t.Helper()
	var response resource.SchemaResponse
	(&paperAccountResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response.Schema
}

func paperAccountTestModel() paperAccountModel {
	return paperAccountModel{
		ID:               types.StringValue(testPaperAccountID),
		StrategyID:       types.StringValue(testPaperAccountStrategyID),
		Purpose:          types.StringValue(testPaperAccountPurpose),
		Name:             types.StringValue(testPaperAccountName),
		InitialCashJpy:   types.Float64Value(1000000),
		BenchmarkStockID: types.StringNull(),
		StartedOn:        types.StringValue(testPaperAccountStartedOn),
	}
}
