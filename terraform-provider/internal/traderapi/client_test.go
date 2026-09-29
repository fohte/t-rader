package traderapi

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"
)

const (
	testStrategyID = "00000000-0000-4000-8000-000000000001"
	testClientID   = "synthetic-client-id"
	testSecret     = "synthetic-client-secret"
)

type observedRequest struct {
	Method       string
	Path         string
	ClientID     string
	ClientSecret string
	Accept       string
	ContentType  string
	Body         string
}

type testHandler func(http.ResponseWriter, *http.Request, []byte)

func TestClientCheckConnection(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusOK)
	})

	err := client.CheckConnection(context.Background())
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Error   string
	}{request, errorMessage(err)}, struct {
		Request observedRequest
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         strategiesPath,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("check connection output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientCreateStrategy(t *testing.T) {
	t.Parallel()

	created := testStrategy()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusCreated, created)
	})
	description := "synthetic description"
	sortOrder := int32(3)

	result, err := client.CreateStrategy(context.Background(), CreateStrategyRequest{
		Name:        created.Name,
		Description: &description,
		SortOrder:   &sortOrder,
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  Strategy
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  Strategy
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPost,
			Path:         strategiesPath,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"name":"synthetic strategy","description":"synthetic description","sort_order":3}` + "\n",
		},
		Result: created,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("create strategy output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateStrategy(t *testing.T) {
	t.Parallel()

	name := "updated synthetic strategy"
	sortOrder := int32(5)
	description := "updated synthetic description"
	descriptionValue := &description
	var noDescription *string

	cases := []struct {
		name    string
		payload UpdateStrategyRequest
		body    string
		result  Strategy
	}{
		{
			name: "omitted description leaves API value unchanged",
			payload: UpdateStrategyRequest{
				Name:      &name,
				SortOrder: &sortOrder,
			},
			body:   `{"name":"updated synthetic strategy","sort_order":5}` + "\n",
			result: testStrategyWithUpdate(),
		},
		{
			name: "null description clears API value",
			payload: UpdateStrategyRequest{
				Description: &noDescription,
			},
			body:   `{"description":null}` + "\n",
			result: testStrategyWithClearedDescription(),
		},
		{
			name: "description value updates API value",
			payload: UpdateStrategyRequest{
				Description: &descriptionValue,
			},
			body:   `{"description":"updated synthetic description"}` + "\n",
			result: testStrategyWithDescription(description),
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
				writeJSON(t, w, http.StatusOK, testCase.result)
			})

			result, err := client.UpdateStrategy(context.Background(), testStrategyID, testCase.payload)
			request := <-requests
			if got, want := struct {
				Request observedRequest
				Result  Strategy
				Error   string
			}{request, result, errorMessage(err)}, struct {
				Request observedRequest
				Result  Strategy
				Error   string
			}{
				Request: observedRequest{
					Method:       http.MethodPatch,
					Path:         strategiesPath + "/" + testStrategyID,
					ClientID:     testClientID,
					ClientSecret: testSecret,
					Accept:       "application/json",
					ContentType:  "application/json",
					Body:         testCase.body,
				},
				Result: testCase.result,
			}; !reflect.DeepEqual(got, want) {
				t.Fatalf("update strategy output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestClientGetStrategy(t *testing.T) {
	t.Parallel()

	strategy := testStrategy()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, strategy)
	})

	result, err := client.GetStrategy(context.Background(), testStrategyID)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  Strategy
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  Strategy
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         strategiesPath + "/" + testStrategyID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: strategy,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("get strategy output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteStrategy(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteStrategy(context.Background(), testStrategyID)
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
			Path:         strategiesPath + "/" + testStrategyID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete strategy output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientReturnsNotFoundSentinel(t *testing.T) {
	t.Parallel()

	client, _ := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		http.Error(w, "synthetic missing strategy", http.StatusNotFound)
	})
	_, err := client.GetStrategy(context.Background(), testStrategyID)
	if got, want := errors.Is(err, ErrNotFound), true; got != want {
		t.Fatalf("not-found result mismatch: got=%t want=%t", got, want)
	}
}

func TestClientRejectsNonUUIDStrategyID(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		t.Errorf("unexpected request for invalid strategy ID")
		w.WriteHeader(http.StatusOK)
	})
	_, err := client.GetStrategy(context.Background(), "not-a-uuid")
	select {
	case request := <-requests:
		t.Fatalf("unexpected request: %#v", request)
	default:
	}
	if got, want := errorMessage(err), "strategy id must be a UUID"; got != want {
		t.Fatalf("invalid ID error mismatch: got=%q want=%q", got, want)
	}
}

func newTestClient(t *testing.T, handler testHandler) (*Client, <-chan observedRequest) {
	t.Helper()

	requests := make(chan observedRequest, 1)
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Errorf("read request body: %v", err)
			return
		}
		requests <- observedRequest{
			Method:       r.Method,
			Path:         r.URL.Path,
			ClientID:     r.Header.Get("CF-Access-Client-Id"),
			ClientSecret: r.Header.Get("CF-Access-Client-Secret"),
			Accept:       r.Header.Get("Accept"),
			ContentType:  r.Header.Get("Content-Type"),
			Body:         string(body),
		}
		handler(w, r, body)
	}))
	t.Cleanup(server.Close)

	client, err := New(server.URL, testClientID, testSecret)
	if err != nil {
		t.Fatalf("create test client: %v", err)
	}
	return client, requests
}

func testStrategy() Strategy {
	description := "synthetic description"
	return Strategy{
		ID:          testStrategyID,
		Name:        "synthetic strategy",
		Description: &description,
		SortOrder:   3,
		CreatedAt:   "2026-01-01T00:00:00Z",
		UpdatedAt:   "2026-01-01T00:00:00Z",
	}
}

func testStrategyWithUpdate() Strategy {
	strategy := testStrategy()
	strategy.Name = "updated synthetic strategy"
	strategy.SortOrder = 5
	strategy.UpdatedAt = "2026-01-02T00:00:00Z"
	return strategy
}

func testStrategyWithClearedDescription() Strategy {
	strategy := testStrategyWithUpdate()
	strategy.Description = nil
	return strategy
}

func testStrategyWithDescription(description string) Strategy {
	strategy := testStrategyWithUpdate()
	strategy.Description = &description
	return strategy
}

func writeJSON(t *testing.T, w http.ResponseWriter, status int, value any) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(value); err != nil {
		t.Errorf("encode response: %v", err)
	}
}

func errorMessage(err error) string {
	if err == nil {
		return ""
	}
	return err.Error()
}
