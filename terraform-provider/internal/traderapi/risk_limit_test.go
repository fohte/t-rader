package traderapi

import (
	"context"
	"net/http"
	"reflect"
	"testing"

	"github.com/oapi-codegen/nullable"
)

func TestClientGetRiskLimit(t *testing.T) {
	t.Parallel()

	expected := AccountRiskPolicyResponse{MaxSectorRatio: nullable.NewNullableWithValue(0.37)}
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, map[string]any{"max_sector_ratio": 0.37})
	})

	result, err := client.GetRiskLimit(context.Background())
	request := <-requests
	got := struct {
		Request observedRequest
		Result  AccountRiskPolicyResponse
		Error   string
	}{Request: request, Result: result, Error: errorMessage(err)}
	want := struct {
		Request observedRequest
		Result  AccountRiskPolicyResponse
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
		payload PutAccountRiskPolicyRequest
		body    string
		result  AccountRiskPolicyResponse
	}{
		{
			name: "null removes the limit",
			payload: PutAccountRiskPolicyRequest{
				MaxSectorRatio: nullable.NewNullNullable[float64](),
			},
			body:   `{"max_sector_ratio":null}`,
			result: AccountRiskPolicyResponse{MaxSectorRatio: nullable.NewNullNullable[float64]()},
		},
		{
			name: "value sets the limit",
			payload: PutAccountRiskPolicyRequest{
				MaxSectorRatio: nullable.NewNullableWithValue(0.37),
			},
			body:   `{"max_sector_ratio":0.37}`,
			result: AccountRiskPolicyResponse{MaxSectorRatio: nullable.NewNullableWithValue(0.37)},
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
				Result  AccountRiskPolicyResponse
				Error   string
			}{Request: request, Result: result, Error: errorMessage(err)}
			want := struct {
				Request observedRequest
				Result  AccountRiskPolicyResponse
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
