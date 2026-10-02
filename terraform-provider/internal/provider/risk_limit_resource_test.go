package provider

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
)

type riskLimitObservedRequest struct {
	Method string
	Path   string
	Body   string
}

type riskLimitTestHandler func(http.ResponseWriter, *http.Request, []byte)

type riskLimitDiagnostic struct {
	Severity diag.Severity
	Summary  string
	Detail   string
}

type riskLimitTestOutput struct {
	Request     *riskLimitObservedRequest
	State       riskLimitModel
	Diagnostics []riskLimitDiagnostic
}

func TestRiskLimitResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := riskLimitResourceSchema(t)
	groupRatios := riskLimitRatios(t,
		riskLimitGroupRatioModel{Axis: types.StringValue("sample-axis"), Ratio: types.Float64Value(0.37)},
		riskLimitGroupRatioModel{Axis: types.StringValue("another-sample-axis"), Ratio: types.Float64Value(0.25)},
	)
	plan := riskLimitPlan(t, resourceSchema.Schema, groupRatios)
	client, requests := newRiskLimitTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeRiskLimitResponse(t, w, http.StatusOK, `{"max_group_ratios":[{"axis":"sample-axis","ratio":0.37},{"axis":"another-sample-axis","ratio":0.25}]}`)
	})

	response := resource.CreateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&riskLimitResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	var state riskLimitModel
	response.Diagnostics.Append(response.State.Get(ctx, &state)...)
	request := <-requests

	got := riskLimitTestOutput{Request: &request, State: state, Diagnostics: riskLimitDiagnostics(response.Diagnostics)}
	want := riskLimitTestOutput{
		Request: &riskLimitObservedRequest{
			Method: http.MethodPut,
			Path:   "/api/account/risk-policy",
			Body:   `{"max_group_ratios":[{"axis":"sample-axis","ratio":0.37},{"axis":"another-sample-axis","ratio":0.25}]}`,
		},
		State: riskLimitModel{ID: types.StringValue(riskLimitResourceID), MaxGroupRatios: groupRatios},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRiskLimitResourceRead(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := riskLimitResourceSchema(t)
	state := riskLimitState(t, resourceSchema.Schema, riskLimitRatios(t,
		riskLimitGroupRatioModel{Axis: types.StringValue("sample-axis"), Ratio: types.Float64Value(0.37)},
	))
	client, requests := newRiskLimitTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeRiskLimitResponse(t, w, http.StatusOK, `{"max_group_ratios":[{"axis":"sample-axis","ratio":0.37},{"axis":"another-sample-axis","ratio":0.25}]}`)
	})

	response := resource.ReadResponse{State: state}
	(&riskLimitResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
	var resultState riskLimitModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	request := <-requests

	got := riskLimitTestOutput{Request: &request, State: resultState, Diagnostics: riskLimitDiagnostics(response.Diagnostics)}
	want := riskLimitTestOutput{
		Request: &riskLimitObservedRequest{Method: http.MethodGet, Path: "/api/account/risk-policy"},
		State: riskLimitModel{
			ID: types.StringValue(riskLimitResourceID),
			MaxGroupRatios: riskLimitRatios(t,
				riskLimitGroupRatioModel{Axis: types.StringValue("sample-axis"), Ratio: types.Float64Value(0.37)},
				riskLimitGroupRatioModel{Axis: types.StringValue("another-sample-axis"), Ratio: types.Float64Value(0.25)},
			),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("read risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRiskLimitResourceUpdateClearsLimit(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := riskLimitResourceSchema(t)
	emptyRatios := emptyRiskLimitGroupRatios()
	plan := riskLimitPlan(t, resourceSchema.Schema, emptyRatios)
	priorState := riskLimitState(t, resourceSchema.Schema, riskLimitRatios(t,
		riskLimitGroupRatioModel{Axis: types.StringValue("sample-axis"), Ratio: types.Float64Value(0.37)},
	))
	client, requests := newRiskLimitTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeRiskLimitResponse(t, w, http.StatusOK, `{"max_group_ratios":[]}`)
	})

	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&riskLimitResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: priorState}, &response)
	var resultState riskLimitModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	request := <-requests

	got := riskLimitTestOutput{Request: &request, State: resultState, Diagnostics: riskLimitDiagnostics(response.Diagnostics)}
	want := riskLimitTestOutput{
		Request: &riskLimitObservedRequest{
			Method: http.MethodPut,
			Path:   "/api/account/risk-policy",
			Body:   `{"max_group_ratios":[]}`,
		},
		State: riskLimitModel{ID: types.StringValue(riskLimitResourceID), MaxGroupRatios: emptyRatios},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRiskLimitResourceDeleteDoesNotCallAPI(t *testing.T) {
	t.Parallel()

	client, requests := newRiskLimitTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusInternalServerError)
	})
	response := resource.DeleteResponse{}
	(&riskLimitResource{client: client}).Delete(context.Background(), resource.DeleteRequest{}, &response)

	var request *riskLimitObservedRequest
	select {
	case observed := <-requests:
		request = &observed
	default:
	}
	got := struct {
		Request     *riskLimitObservedRequest
		Diagnostics []riskLimitDiagnostic
	}{Request: request, Diagnostics: riskLimitDiagnostics(response.Diagnostics)}
	want := struct {
		Request     *riskLimitObservedRequest
		Diagnostics []riskLimitDiagnostic
	}{}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRiskLimitResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := riskLimitResourceSchema(t)
	plan := riskLimitPlan(t, resourceSchema.Schema, emptyRiskLimitGroupRatios())
	response := resource.ImportStateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&riskLimitResource{}).ImportState(ctx, resource.ImportStateRequest{ID: riskLimitResourceID}, &response)
	var id types.String
	response.Diagnostics.Append(response.State.GetAttribute(ctx, path.Root("id"), &id)...)

	got := struct {
		ID          types.String
		Diagnostics []riskLimitDiagnostic
	}{ID: id, Diagnostics: riskLimitDiagnostics(response.Diagnostics)}
	want := struct {
		ID          types.String
		Diagnostics []riskLimitDiagnostic
	}{ID: types.StringValue(riskLimitResourceID)}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestRiskLimitResourceRejectsUnknownImportID(t *testing.T) {
	t.Parallel()

	response := resource.ImportStateResponse{}
	(&riskLimitResource{}).ImportState(context.Background(), resource.ImportStateRequest{ID: "other"}, &response)
	got := riskLimitDiagnostics(response.Diagnostics)
	want := []riskLimitDiagnostic{{
		Severity: diag.SeverityError,
		Summary:  "Invalid Import ID",
		Detail:   `Risk limit import ID must be "account".`,
	}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import ID diagnostics mismatch: got=%#v want=%#v", got, want)
	}
}

func TestMaxGroupRatioValidator(t *testing.T) {
	t.Parallel()

	ratioPath := path.Root("max_group_ratios").AtListIndex(0).AtName("ratio")
	invalidRatio := diag.Diagnostics{diag.NewAttributeErrorDiagnostic(
		ratioPath,
		"Invalid group ratio",
		"ratio must be greater than 0 and less than or equal to 1.",
	)}
	cases := []struct {
		name  string
		value types.Float64
		want  diag.Diagnostics
	}{
		{name: "null removes the limit", value: types.Float64Null()},
		{name: "unknown value is deferred", value: types.Float64Unknown()},
		{name: "value inside range is accepted", value: types.Float64Value(0.37)},
		{name: "one is accepted", value: types.Float64Value(1)},
		{name: "zero is rejected", value: types.Float64Value(0), want: invalidRatio},
		{name: "negative value is rejected", value: types.Float64Value(-0.1), want: invalidRatio},
		{name: "value above one is rejected", value: types.Float64Value(1.1), want: invalidRatio},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			var response validator.Float64Response
			(maxGroupRatioValidator{}).ValidateFloat64(context.Background(), validator.Float64Request{
				Path:        ratioPath,
				ConfigValue: testCase.value,
			}, &response)
			if !response.Diagnostics.Equal(testCase.want) {
				t.Fatalf("ratio validator diagnostics mismatch: got=%#v want=%#v", response.Diagnostics, testCase.want)
			}
		})
	}
}

func riskLimitResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&riskLimitResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func riskLimitPlan(t *testing.T, resourceSchema schema.Schema, groupRatios types.List) tfsdk.Plan {
	t.Helper()
	plan := tfsdk.Plan{Schema: resourceSchema}
	diagnostics := plan.Set(context.Background(), riskLimitModel{
		ID:             types.StringUnknown(),
		MaxGroupRatios: groupRatios,
	})
	if diagnostics.HasError() {
		t.Fatalf("build risk limit plan: %v", diagnostics)
	}
	return plan
}

func riskLimitState(t *testing.T, resourceSchema schema.Schema, groupRatios types.List) tfsdk.State {
	t.Helper()
	state := tfsdk.State{Schema: resourceSchema}
	diagnostics := state.Set(context.Background(), riskLimitModel{
		ID:             types.StringValue(riskLimitResourceID),
		MaxGroupRatios: groupRatios,
	})
	if diagnostics.HasError() {
		t.Fatalf("build risk limit state: %v", diagnostics)
	}
	return state
}

func riskLimitRatios(t *testing.T, values ...riskLimitGroupRatioModel) types.List {
	t.Helper()
	groupRatios, diagnostics := types.ListValueFrom(context.Background(), riskLimitGroupRatioObjectType(), values)
	if diagnostics.HasError() {
		t.Fatalf("build risk limit group ratios: %v", diagnostics)
	}
	return groupRatios
}

func newRiskLimitTestClient(t *testing.T, handler riskLimitTestHandler) (*traderapi.Client, <-chan riskLimitObservedRequest) {
	t.Helper()

	requests := make(chan riskLimitObservedRequest, 1)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Errorf("read request body: %v", err)
			return
		}
		requests <- riskLimitObservedRequest{Method: r.Method, Path: r.URL.Path, Body: string(body)}
		handler(w, r, body)
	}))
	t.Cleanup(server.Close)

	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create risk limit test client: %v", err)
	}
	return client, requests
}

func writeRiskLimitResponse(t *testing.T, w http.ResponseWriter, status int, body string) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if _, err := io.WriteString(w, body); err != nil {
		t.Errorf("write risk limit response: %v", err)
	}
}

func riskLimitDiagnostics(diagnostics diag.Diagnostics) []riskLimitDiagnostic {
	var result []riskLimitDiagnostic
	for _, diagnostic := range diagnostics {
		result = append(result, riskLimitDiagnostic{
			Severity: diagnostic.Severity(),
			Summary:  diagnostic.Summary(),
			Detail:   diagnostic.Detail(),
		})
	}
	return result
}
