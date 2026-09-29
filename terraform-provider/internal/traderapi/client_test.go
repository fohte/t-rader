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

func TestClientStrategyLifecycle(t *testing.T) {
	t.Parallel()

	const strategyID = "00000000-0000-4000-8000-000000000001"
	const clientID = "synthetic-client-id"
	const clientSecret = "synthetic-client-secret"
	const description = "synthetic description"

	created := Strategy{
		ID:          strategyID,
		Name:        "synthetic strategy",
		Description: stringPointer(description),
		SortOrder:   3,
		CreatedAt:   "2026-01-01T00:00:00Z",
		UpdatedAt:   "2026-01-01T00:00:00Z",
	}
	updated := created
	updated.Name = "updated synthetic strategy"
	updated.SortOrder = 5
	updated.UpdatedAt = "2026-01-02T00:00:00Z"
	cleared := updated
	cleared.Description = nil

	type observedRequest struct {
		Method       string
		Path         string
		ClientID     string
		ClientSecret string
		ContentType  string
		Body         string
	}
	var observed []observedRequest

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			t.Errorf("read request body: %v", err)
			return
		}
		observed = append(observed, observedRequest{
			Method:       r.Method,
			Path:         r.URL.Path,
			ClientID:     r.Header.Get("CF-Access-Client-Id"),
			ClientSecret: r.Header.Get("CF-Access-Client-Secret"),
			ContentType:  r.Header.Get("Content-Type"),
			Body:         string(body),
		})

		switch {
		case r.Method == http.MethodGet && r.URL.Path == strategiesPath:
			w.WriteHeader(http.StatusOK)
		case r.Method == http.MethodPost && r.URL.Path == strategiesPath:
			writeJSON(t, w, http.StatusCreated, created)
		case r.Method == http.MethodPatch && r.URL.Path == strategiesPath+"/"+strategyID:
			if string(body) == "{\"description\":null}\n" {
				writeJSON(t, w, http.StatusOK, cleared)
			} else {
				writeJSON(t, w, http.StatusOK, updated)
			}
		case r.Method == http.MethodGet && r.URL.Path == strategiesPath+"/"+strategyID:
			writeJSON(t, w, http.StatusOK, cleared)
		case r.Method == http.MethodDelete && r.URL.Path == strategiesPath+"/"+strategyID:
			w.WriteHeader(http.StatusNoContent)
		default:
			t.Errorf("unexpected request: %s %s", r.Method, r.URL.Path)
			http.NotFound(w, r)
		}
	}))
	defer server.Close()

	client, err := New(server.URL, clientID, clientSecret)
	if err != nil {
		t.Fatal(err)
	}
	if err := client.CheckConnection(context.Background()); err != nil {
		t.Fatal(err)
	}
	createdResult, err := client.CreateStrategy(context.Background(), CreateStrategyRequest{
		Name:        created.Name,
		Description: stringPointer(description),
		SortOrder:   int32Pointer(3),
	})
	if err != nil {
		t.Fatal(err)
	}
	updatedResult, err := client.UpdateStrategy(context.Background(), strategyID, UpdateStrategyRequest{
		Name:      stringPointer(updated.Name),
		SortOrder: int32Pointer(5),
	})
	if err != nil {
		t.Fatal(err)
	}
	var nullDescription *string
	clearedResult, err := client.UpdateStrategy(context.Background(), strategyID, UpdateStrategyRequest{
		Description: &nullDescription,
	})
	if err != nil {
		t.Fatal(err)
	}
	readResult, err := client.GetStrategy(context.Background(), strategyID)
	if err != nil {
		t.Fatal(err)
	}
	if err := client.DeleteStrategy(context.Background(), strategyID); err != nil {
		t.Fatal(err)
	}

	wantObserved := []observedRequest{
		{Method: http.MethodGet, Path: strategiesPath, ClientID: clientID, ClientSecret: clientSecret},
		{Method: http.MethodPost, Path: strategiesPath, ClientID: clientID, ClientSecret: clientSecret, ContentType: "application/json", Body: `{"name":"synthetic strategy","description":"synthetic description","sort_order":3}` + "\n"},
		{Method: http.MethodPatch, Path: strategiesPath + "/" + strategyID, ClientID: clientID, ClientSecret: clientSecret, ContentType: "application/json", Body: `{"name":"updated synthetic strategy","sort_order":5}` + "\n"},
		{Method: http.MethodPatch, Path: strategiesPath + "/" + strategyID, ClientID: clientID, ClientSecret: clientSecret, ContentType: "application/json", Body: `{"description":null}` + "\n"},
		{Method: http.MethodGet, Path: strategiesPath + "/" + strategyID, ClientID: clientID, ClientSecret: clientSecret},
		{Method: http.MethodDelete, Path: strategiesPath + "/" + strategyID, ClientID: clientID, ClientSecret: clientSecret},
	}
	wantResults := []Strategy{created, updated, cleared, cleared}
	if !reflect.DeepEqual(observed, wantObserved) || !reflect.DeepEqual([]Strategy{createdResult, updatedResult, clearedResult, readResult}, wantResults) {
		t.Fatalf("lifecycle output mismatch: observed=%#v results=%#v", observed, []Strategy{createdResult, updatedResult, clearedResult, readResult})
	}
}

func TestClientReturnsNotFoundSentinel(t *testing.T) {
	t.Parallel()

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		http.Error(w, "missing", http.StatusNotFound)
	}))
	defer server.Close()

	client, err := New(server.URL, "", "")
	if err != nil {
		t.Fatal(err)
	}
	_, err = client.GetStrategy(context.Background(), "00000000-0000-4000-8000-000000000001")
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("expected not-found error, got %v", err)
	}
}

func TestClientRejectsNonUUIDStrategyID(t *testing.T) {
	t.Parallel()

	client, err := New("http://localhost:3000", "", "")
	if err != nil {
		t.Fatal(err)
	}
	_, err = client.GetStrategy(context.Background(), "not-a-uuid")
	if err == nil {
		t.Fatal("expected an invalid ID error")
	}
}

func writeJSON(t *testing.T, w http.ResponseWriter, status int, value any) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(value); err != nil {
		t.Errorf("encode response: %v", err)
	}
}

func stringPointer(value string) *string {
	return &value
}

func int32Pointer(value int32) *int32 {
	return &value
}
