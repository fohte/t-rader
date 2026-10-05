package traderapi

import (
	"context"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
	"net/http"
	"reflect"
	"testing"
)

func TestClientGetRiskLimit(t *testing.T) {
	t.Parallel()

	expected := gen.AccountRiskPolicyResponse{MaxGroupRatios: []gen.GroupRatio{{Axis: "sample-axis", Ratio: 0.37}}}
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, map[string]any{
			"max_group_ratios": []map[string]any{{"axis": "sample-axis", "ratio": 0.37}},
		})
	})

	result, err := client.GetRiskLimit(context.Background())
	request := <-requests
	got := struct {
		Request observedRequest
		Result  gen.AccountRiskPolicyResponse
		Error   string
	}{Request: request, Result: result, Error: errorMessage(err)}
	want := struct {
		Request observedRequest
		Result  gen.AccountRiskPolicyResponse
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/account/risk-policy",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: expected,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("get risk limit output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientPutRiskLimit(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name    string
		payload gen.PutAccountRiskPolicyRequest
		body    string
		result  gen.AccountRiskPolicyResponse
	}{
		{
			name: "empty list removes all limits",
			payload: gen.PutAccountRiskPolicyRequest{
				MaxGroupRatios: []gen.GroupRatio{},
			},
			body:   `{"max_group_ratios":[]}`,
			result: gen.AccountRiskPolicyResponse{MaxGroupRatios: []gen.GroupRatio{}},
		},
		{
			name: "multiple ratios set limits",
			payload: gen.PutAccountRiskPolicyRequest{
				MaxGroupRatios: []gen.GroupRatio{
					{Axis: "sample-axis", Ratio: 0.37},
					{Axis: "another-sample-axis", Ratio: 0.2},
				},
			},
			body: `{"max_group_ratios":[{"axis":"sample-axis","ratio":0.37},{"axis":"another-sample-axis","ratio":0.2}]}`,
			result: gen.AccountRiskPolicyResponse{MaxGroupRatios: []gen.GroupRatio{
				{Axis: "sample-axis", Ratio: 0.37},
				{Axis: "another-sample-axis", Ratio: 0.2},
			}},
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
				writeJSON(t, w, http.StatusOK, testCase.result)
			})

			result, err := client.PutRiskLimit(context.Background(), testCase.payload)
			request := <-requests
			got := struct {
				Request observedRequest
				Result  gen.AccountRiskPolicyResponse
				Error   string
			}{Request: request, Result: result, Error: errorMessage(err)}
			want := struct {
				Request observedRequest
				Result  gen.AccountRiskPolicyResponse
				Error   string
			}{
				Request: observedRequest{
					Method:       http.MethodPut,
					Path:         "/api/account/risk-policy",
					ClientID:     testClientID,
					ClientSecret: testSecret,
					Accept:       "application/json",
					ContentType:  "application/json",
					Body:         testCase.body,
				},
				Result: testCase.result,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("put risk limit output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}
