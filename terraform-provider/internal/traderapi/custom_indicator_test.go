package traderapi

import (
	"context"
	"net/http"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/oapi-codegen/nullable"
)

const testCustomIndicatorID = "00000000-0000-4000-8000-000000000002"

func TestClientCreateCustomIndicator(t *testing.T) {
	t.Parallel()

	strategyID := testStrategyID
	cases := []struct {
		name       string
		strategyID *string
		path       string
		scope      string
	}{
		{
			name:  "global indicator uses the global endpoint",
			path:  "/api/indicators",
			scope: "global",
		},
		{
			name:       "strategy indicator uses the strategy endpoint",
			strategyID: &strategyID,
			path:       "/api/strategies/" + testStrategyID + "/indicators",
			scope:      "strategy",
		},
	}

	for _, testCase := range cases {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			indicator := testCustomIndicator(testCase.scope, testCase.strategyID, nullable.NewNullableWithValue("synthetic description"))
			client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
				writeJSON(t, w, http.StatusCreated, indicator)
			})
			payload := CreateCustomIndicatorRequest{
				Code:         "synthetic_code",
				Description:  nullable.NewNullableWithValue("synthetic description"),
				InputSchema:  map[string]interface{}{},
				Name:         "synthetic indicator",
				OutputSchema: map[string]interface{}{},
			}

			result, err := client.CreateCustomIndicator(context.Background(), testCase.strategyID, payload)
			request := <-requests
			if got, want := struct {
				Request observedRequest
				Result  CustomIndicator
				Error   string
			}{request, result, errorMessage(err)}, struct {
				Request observedRequest
				Result  CustomIndicator
				Error   string
			}{
				Request: observedRequest{
					Method:       http.MethodPost,
					Path:         testCase.path,
					ClientID:     testClientID,
					ClientSecret: testSecret,
					Accept:       "application/json",
					ContentType:  "application/json",
					Body:         `{"code":"synthetic_code","description":"synthetic description","input_schema":{},"name":"synthetic indicator","output_schema":{}}`,
				},
				Result: indicator,
			}; !reflect.DeepEqual(got, want) {
				t.Fatalf("create custom indicator output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestClientGetCustomIndicator(t *testing.T) {
	t.Parallel()

	indicator := testCustomIndicator("global", nil, nullable.NewNullableWithValue("synthetic description"))
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, indicator)
	})

	result, err := client.GetCustomIndicator(context.Background(), testCustomIndicatorID)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  CustomIndicator
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  CustomIndicator
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/indicators/" + testCustomIndicatorID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: indicator,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("get custom indicator output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateCustomIndicatorClearsDescription(t *testing.T) {
	t.Parallel()

	indicator := testCustomIndicator("global", nil, nullable.NewNullNullable[string]())
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, indicator)
	})

	result, err := client.UpdateCustomIndicator(context.Background(), testCustomIndicatorID, UpdateCustomIndicatorRequest{
		Description: nullable.NewNullNullable[string](),
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  CustomIndicator
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  CustomIndicator
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPut,
			Path:         "/api/indicators/" + testCustomIndicatorID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"description":null}`,
		},
		Result: indicator,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("update custom indicator output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteCustomIndicator(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteCustomIndicator(context.Background(), testCustomIndicatorID)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Error   string
	}{request, errorMessage(err)}, struct {
		Request observedRequest
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodDelete,
			Path:         "/api/indicators/" + testCustomIndicatorID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete custom indicator output mismatch: got=%#v want=%#v", got, want)
	}
}

func testCustomIndicator(scope string, strategyID *string, description nullable.Nullable[string]) CustomIndicator {
	indicator := CustomIndicator{
		Code:         "synthetic_code",
		CreatedAt:    time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		Description:  description,
		IndicatorId:  uuid.MustParse(testCustomIndicatorID),
		InputSchema:  map[string]interface{}{},
		Name:         "synthetic indicator",
		OutputSchema: map[string]interface{}{},
		Scope:        scope,
		UpdatedAt:    time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
	}
	if strategyID == nil {
		indicator.StrategyId = nullable.NewNullNullable[uuid.UUID]()
	} else {
		indicator.StrategyId = nullable.NewNullableWithValue(uuid.MustParse(*strategyID))
	}
	return indicator
}
