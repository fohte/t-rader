package traderapi

import (
	"context"
	"errors"
	"net/http"
	"reflect"
	"testing"
	"time"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

const testDashboardKey = "sample-dashboard"

const testDashboardConfig = "title: sample-dashboard\nwidgets: []\n"

func TestClientCreateDashboard(t *testing.T) {
	t.Parallel()

	created := testDashboard()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusCreated, created)
	})

	result, err := client.CreateDashboard(context.Background(), gen.CreateDashboardRequest{
		Key:    testDashboardKey,
		Config: testDashboardConfig,
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPost,
			Path:         "/api/dashboards",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"config":"title: sample-dashboard\nwidgets: []\n","key":"sample-dashboard"}`,
		},
		Result: created,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("create dashboard output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientGetDashboard(t *testing.T) {
	t.Parallel()

	wantDashboard := testDashboard()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, wantDashboard)
	})

	result, err := client.GetDashboard(context.Background(), testDashboardKey)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/dashboards/" + testDashboardKey,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: wantDashboard,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("get dashboard output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateDashboard(t *testing.T) {
	t.Parallel()

	updated := testDashboard()
	updated.Config = "title: updated-dashboard\nwidgets: []\n"
	updated.UpdatedAt = time.Date(2026, time.January, 2, 0, 0, 0, 0, time.UTC)
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, updated)
	})

	result, err := client.UpdateDashboard(context.Background(), testDashboardKey, gen.UpdateDashboardRequest{
		Config: updated.Config,
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.Dashboard
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPut,
			Path:         "/api/dashboards/" + testDashboardKey,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"config":"title: updated-dashboard\nwidgets: []\n"}`,
		},
		Result: updated,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("update dashboard output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteDashboard(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteDashboard(context.Background(), testDashboardKey)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Error   string
	}{request, errorMessage(err)}, struct {
		Request observedRequest
		Error   string
	}{Request: observedRequest{
		Method:       http.MethodDelete,
		Path:         "/api/dashboards/" + testDashboardKey,
		ClientID:     testClientID,
		ClientSecret: testSecret,
		Accept:       "application/json",
	}}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete dashboard output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientReturnsNotFoundForMissingDashboard(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusNotFound, gen.ErrorResponse{Error: "dashboard is missing"})
	})
	_, err := client.GetDashboard(context.Background(), testDashboardKey)
	request := <-requests
	if got, want := struct {
		Request  observedRequest
		Error    string
		NotFound bool
	}{request, errorMessage(err), errors.Is(err, ErrNotFound)}, struct {
		Request  observedRequest
		Error    string
		NotFound bool
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/dashboards/" + testDashboardKey,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Error:    "resource not found: backend returned HTTP 404: dashboard is missing",
		NotFound: true,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("missing dashboard output mismatch: got=%#v want=%#v", got, want)
	}
}

func testDashboard() gen.Dashboard {
	return gen.Dashboard{
		Config:    testDashboardConfig,
		Key:       testDashboardKey,
		CreatedAt: time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		UpdatedAt: time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
	}
}
