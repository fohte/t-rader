package provider

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi"
)

type noteKindRequestObservation struct {
	Method string
	Path   string
	Body   string
}

type noteKindDiagnosticObservation struct {
	Severity string
	Summary  string
	Detail   string
	Path     string
}

func TestNoteKindResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	kind := syntheticNoteKind()
	requests := make(chan noteKindRequestObservation, 1)
	client := newNoteKindTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordNoteKindRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		if err := json.NewEncoder(w).Encode(kind); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := noteKindResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, noteKindModel{
		Key:              types.StringValue("synthetic_kind"),
		DisplayName:      types.StringValue("Synthetic Kind"),
		RequiresApproval: types.BoolValue(true),
		Description:      types.StringValue("synthetic description"),
		SortOrder:        types.Int32Value(3),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build create plan: %v", planDiagnostics)
	}

	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	(&noteKindResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	observedRequest := receiveNoteKindRequest(requests)
	var resultState noteKindModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	type output struct {
		Request     *noteKindRequestObservation
		State       noteKindModel
		Diagnostics []noteKindDiagnosticObservation
	}
	got := output{
		Request:     observedRequest,
		State:       resultState,
		Diagnostics: noteKindDiagnosticsOutput(response.Diagnostics),
	}
	want := output{
		Request: &noteKindRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/note-kinds",
			Body:   `{"description":"synthetic description","display_name":"Synthetic Kind","key":"synthetic_kind","requires_approval":true,"sort_order":3}`,
		},
		State:       modelFromNoteKind(kind),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteKindResourceRead(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name         string
		responseBody any
		wantRemoved  bool
		wantState    noteKindModel
	}{
		{
			name:         "finds the key in the list",
			responseBody: []traderapi.NoteKind{syntheticNoteKind()},
			wantState:    modelFromNoteKind(syntheticNoteKind()),
		},
		{
			name:         "removes a key absent from the list",
			responseBody: []traderapi.NoteKind{},
			wantRemoved:  true,
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			requests := make(chan noteKindRequestObservation, 1)
			client := newNoteKindTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				if !recordNoteKindRequest(t, requests, r) {
					return
				}
				w.Header().Set("Content-Type", "application/json")
				if err := json.NewEncoder(w).Encode(testCase.responseBody); err != nil {
					t.Errorf("encode response: %v", err)
				}
			})
			resourceSchema := noteKindResourceSchema(t)
			state := tfsdk.State{Schema: resourceSchema.Schema}
			stateDiagnostics := state.Set(ctx, syntheticNoteKindModel())
			if stateDiagnostics.HasError() {
				t.Fatalf("build prior state: %v", stateDiagnostics)
			}

			response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}
			(&noteKindResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
			observedRequest := receiveNoteKindRequest(requests)
			var resultState noteKindModel
			if !testCase.wantRemoved {
				response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
			}

			got := struct {
				Request      *noteKindRequestObservation
				State        noteKindModel
				StateRemoved bool
				Diagnostics  []noteKindDiagnosticObservation
			}{
				Request:      observedRequest,
				State:        resultState,
				StateRemoved: response.State.Raw.IsNull(),
				Diagnostics:  noteKindDiagnosticsOutput(response.Diagnostics),
			}
			want := struct {
				Request      *noteKindRequestObservation
				State        noteKindModel
				StateRemoved bool
				Diagnostics  []noteKindDiagnosticObservation
			}{
				Request:      &noteKindRequestObservation{Method: http.MethodGet, Path: "/api/note-kinds"},
				State:        testCase.wantState,
				StateRemoved: testCase.wantRemoved,
			}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestNoteKindResourceUpdate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	updated := syntheticNoteKind()
	updated.Description = nullable.NewNullNullable[string]()
	updated.DisplayName = "Updated Synthetic Kind"
	updated.RequiresApproval = false
	updated.SortOrder = 5
	requests := make(chan noteKindRequestObservation, 1)
	client := newNoteKindTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordNoteKindRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		if err := json.NewEncoder(w).Encode(updated); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := noteKindResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticNoteKindModel())
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, noteKindModel{
		Key:              types.StringValue("synthetic_kind"),
		DisplayName:      types.StringValue("Updated Synthetic Kind"),
		RequiresApproval: types.BoolValue(false),
		Description:      types.StringNull(),
		SortOrder:        types.Int32Value(5),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build update plan: %v", planDiagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&noteKindResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	observedRequest := receiveNoteKindRequest(requests)
	var resultState noteKindModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)

	got := struct {
		Request     *noteKindRequestObservation
		State       noteKindModel
		Diagnostics []noteKindDiagnosticObservation
	}{Request: observedRequest, State: resultState, Diagnostics: noteKindDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *noteKindRequestObservation
		State       noteKindModel
		Diagnostics []noteKindDiagnosticObservation
	}{
		Request: &noteKindRequestObservation{
			Method: http.MethodPatch,
			Path:   "/api/note-kinds/synthetic_kind",
			Body:   `{"description":null,"display_name":"Updated Synthetic Kind","requires_approval":false,"sort_order":5}`,
		},
		State:       modelFromNoteKind(updated),
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteKindResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan noteKindRequestObservation, 1)
	client := newNoteKindTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordNoteKindRequest(t, requests, r) {
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	resourceSchema := noteKindResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticNoteKindModel())
	if stateDiagnostics.HasError() {
		t.Fatalf("build delete state: %v", stateDiagnostics)
	}

	response := resource.DeleteResponse{}
	(&noteKindResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	observedRequest := receiveNoteKindRequest(requests)
	got := struct {
		Request     *noteKindRequestObservation
		Diagnostics []noteKindDiagnosticObservation
	}{Request: observedRequest, Diagnostics: noteKindDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *noteKindRequestObservation
		Diagnostics []noteKindDiagnosticObservation
	}{
		Request:     &noteKindRequestObservation{Method: http.MethodDelete, Path: "/api/note-kinds/synthetic_kind"},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteKindResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := noteKindResourceSchema(t)
	response := resource.ImportStateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	stateDiagnostics := response.State.Set(ctx, noteKindModel{
		Key:              types.StringNull(),
		DisplayName:      types.StringNull(),
		RequiresApproval: types.BoolNull(),
		Description:      types.StringNull(),
		SortOrder:        types.Int32Null(),
	})
	if stateDiagnostics.HasError() {
		t.Fatalf("build import state: %v", stateDiagnostics)
	}

	(&noteKindResource{}).ImportState(ctx, resource.ImportStateRequest{ID: "synthetic_kind"}, &response)
	var imported noteKindModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)
	got := struct {
		State       noteKindModel
		Diagnostics []noteKindDiagnosticObservation
	}{State: imported, Diagnostics: noteKindDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       noteKindModel
		Diagnostics []noteKindDiagnosticObservation
	}{
		State: noteKindModel{
			Key:              types.StringValue("synthetic_kind"),
			DisplayName:      types.StringNull(),
			RequiresApproval: types.BoolNull(),
			Description:      types.StringNull(),
			SortOrder:        types.Int32Null(),
		},
		Diagnostics: nil,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteKindResourceKeyRequiresReplacement(t *testing.T) {
	t.Parallel()

	resourceSchema := noteKindResourceSchema(t)
	attribute := resourceSchema.Schema.Attributes["key"].(schema.StringAttribute)
	ctx := context.Background()
	state := tfsdk.State{Schema: resourceSchema.Schema}
	stateDiagnostics := state.Set(ctx, syntheticNoteKindModel())
	if stateDiagnostics.HasError() {
		t.Fatalf("build prior state: %v", stateDiagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	planDiagnostics := plan.Set(ctx, noteKindModel{
		Key:              types.StringValue("renamed_synthetic_kind"),
		DisplayName:      types.StringValue("Synthetic Kind"),
		RequiresApproval: types.BoolValue(true),
		Description:      types.StringValue("synthetic description"),
		SortOrder:        types.Int32Value(3),
	})
	if planDiagnostics.HasError() {
		t.Fatalf("build plan: %v", planDiagnostics)
	}
	request := planmodifier.StringRequest{
		Path:        path.Root("key"),
		State:       state,
		Plan:        plan,
		StateValue:  types.StringValue("synthetic_kind"),
		PlanValue:   types.StringValue("renamed_synthetic_kind"),
		ConfigValue: types.StringValue("renamed_synthetic_kind"),
	}
	response := planmodifier.StringResponse{PlanValue: request.PlanValue}
	attribute.PlanModifiers[0].PlanModifyString(ctx, request, &response)

	got := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []noteKindDiagnosticObservation
	}{PlanValue: response.PlanValue, RequiresReplace: response.RequiresReplace, Diagnostics: noteKindDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []noteKindDiagnosticObservation
	}{PlanValue: types.StringValue("renamed_synthetic_kind"), RequiresReplace: true}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("key plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func newNoteKindTestClient(t *testing.T, handler http.HandlerFunc) *traderapi.Client {
	t.Helper()
	server := httptest.NewServer(handler)
	t.Cleanup(server.Close)
	client, err := traderapi.New(server.URL, "", "")
	if err != nil {
		t.Fatalf("create client: %v", err)
	}
	return client
}

func recordNoteKindRequest(t *testing.T, requests chan<- noteKindRequestObservation, request *http.Request) bool {
	t.Helper()
	body, err := io.ReadAll(request.Body)
	if err != nil {
		t.Errorf("read request body: %v", err)
		return false
	}
	requests <- noteKindRequestObservation{Method: request.Method, Path: request.URL.Path, Body: string(body)}
	return true
}

func receiveNoteKindRequest(requests <-chan noteKindRequestObservation) *noteKindRequestObservation {
	observed := <-requests
	return &observed
}

func noteKindResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&noteKindResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func syntheticNoteKind() traderapi.NoteKind {
	return traderapi.NoteKind{
		Description:      nullable.NewNullableWithValue("synthetic description"),
		DisplayName:      "Synthetic Kind",
		Key:              "synthetic_kind",
		RequiresApproval: true,
		SortOrder:        3,
	}
}

func syntheticNoteKindModel() noteKindModel {
	return modelFromNoteKind(syntheticNoteKind())
}

func noteKindDiagnosticsOutput(diagnostics diag.Diagnostics) []noteKindDiagnosticObservation {
	var result []noteKindDiagnosticObservation
	for _, diagnostic := range diagnostics {
		observation := noteKindDiagnosticObservation{
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
