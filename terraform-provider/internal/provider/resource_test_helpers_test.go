package provider

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/diag"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

type apiRequestObservation struct {
	Method string
	Path   string
	Body   string
}

type apiDiagnosticObservation struct {
	Severity string
	Summary  string
	Detail   string
	Path     string
}

func newAPIResourceTestClient(t *testing.T, handler http.HandlerFunc) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}
	return client
}

func recordAPIResourceRequest(t *testing.T, requests chan<- apiRequestObservation, request *http.Request) bool {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
		return false
	}
	requests <- apiRequestObservation{Method: request.Method, Path: request.URL.Path, Body: string(body)}
	return true
}

func receiveAPIResourceRequest(requests <-chan apiRequestObservation) *apiRequestObservation {
	observed := <-requests
	return &observed
}

func apiResourceDiagnosticsOutput(diagnostics diag.Diagnostics) []apiDiagnosticObservation {
	var result []apiDiagnosticObservation
	for _, diagnostic := range diagnostics {
		observation := apiDiagnosticObservation{
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
