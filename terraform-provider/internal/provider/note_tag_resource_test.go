package provider

import (
	"context"
	"encoding/json"
	"net/http"
	"reflect"
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/tfsdk"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"

	traderapigen "github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func TestNoteTagResourceCreate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	tag := syntheticNoteTag()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		if err := json.NewEncoder(w).Encode(tag); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := noteTagResourceSchema(t)
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, noteTagModel{
		Key:         types.StringValue("synthetic_tag"),
		DisplayName: types.StringValue("Synthetic Tag"),
		Description: types.StringValue("synthetic description"),
	}); diagnostics.HasError() {
		t.Fatalf("build create plan: %v", diagnostics)
	}

	response := resource.CreateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	(&noteTagResource{client: client}).Create(ctx, resource.CreateRequest{Plan: plan}, &response)
	request := receiveAPIResourceRequest(requests)
	var resultState noteTagModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	got := struct {
		Request     *apiRequestObservation
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{Request: request, State: resultState, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPost,
			Path:   "/api/note-tags",
			Body:   `{"description":"synthetic description","display_name":"Synthetic Tag","key":"synthetic_tag"}`,
		},
		State: modelFromNoteTag(tag),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("create output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteTagResourceRead(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name        string
		response    []traderapigen.NoteTag
		wantRemoved bool
		wantState   noteTagModel
	}{
		{name: "finds the key in the list", response: []traderapigen.NoteTag{syntheticNoteTag()}, wantState: modelFromNoteTag(syntheticNoteTag())},
		{name: "removes a key absent from the list", response: []traderapigen.NoteTag{}, wantRemoved: true},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			ctx := context.Background()
			requests := make(chan apiRequestObservation, 1)
			client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
				if !recordAPIResourceRequest(t, requests, r) {
					return
				}
				w.Header().Set("Content-Type", "application/json")
				if err := json.NewEncoder(w).Encode(testCase.response); err != nil {
					t.Errorf("encode response: %v", err)
				}
			})
			resourceSchema := noteTagResourceSchema(t)
			state := tfsdk.State{Schema: resourceSchema.Schema}
			if diagnostics := state.Set(ctx, syntheticNoteTagModel()); diagnostics.HasError() {
				t.Fatalf("build prior state: %v", diagnostics)
			}

			response := resource.ReadResponse{State: tfsdk.State{Raw: state.Raw, Schema: resourceSchema.Schema}}
			(&noteTagResource{client: client}).Read(ctx, resource.ReadRequest{State: state}, &response)
			request := receiveAPIResourceRequest(requests)
			var resultState noteTagModel
			if !testCase.wantRemoved {
				response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
			}
			got := struct {
				Request      *apiRequestObservation
				State        noteTagModel
				StateRemoved bool
				Diagnostics  []apiDiagnosticObservation
			}{Request: request, State: resultState, StateRemoved: response.State.Raw.IsNull(), Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
			want := struct {
				Request      *apiRequestObservation
				State        noteTagModel
				StateRemoved bool
				Diagnostics  []apiDiagnosticObservation
			}{Request: &apiRequestObservation{Method: http.MethodGet, Path: "/api/note-tags"}, State: testCase.wantState, StateRemoved: testCase.wantRemoved}
			if !reflect.DeepEqual(got, want) {
				t.Fatalf("read output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func TestNoteTagResourceUpdate(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	updated := syntheticNoteTag()
	updated.Description = nullable.NewNullNullable[string]()
	updated.DisplayName = "Updated Synthetic Tag"
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		w.Header().Set("Content-Type", "application/json")
		if err := json.NewEncoder(w).Encode(updated); err != nil {
			t.Errorf("encode response: %v", err)
		}
	})
	resourceSchema := noteTagResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, syntheticNoteTagModel()); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, noteTagModel{
		Key:         types.StringValue("synthetic_tag"),
		DisplayName: types.StringValue("Updated Synthetic Tag"),
		Description: types.StringNull(),
	}); diagnostics.HasError() {
		t.Fatalf("build update plan: %v", diagnostics)
	}

	response := resource.UpdateResponse{State: tfsdk.State{Raw: plan.Raw, Schema: resourceSchema.Schema}}
	(&noteTagResource{client: client}).Update(ctx, resource.UpdateRequest{Plan: plan, State: state}, &response)
	request := receiveAPIResourceRequest(requests)
	var resultState noteTagModel
	response.Diagnostics.Append(response.State.Get(ctx, &resultState)...)
	got := struct {
		Request     *apiRequestObservation
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{Request: request, State: resultState, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{
		Request: &apiRequestObservation{
			Method: http.MethodPatch,
			Path:   "/api/note-tags/synthetic_tag",
			Body:   `{"description":null,"display_name":"Updated Synthetic Tag"}`,
		},
		State: modelFromNoteTag(updated),
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("update output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteTagResourceDelete(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	requests := make(chan apiRequestObservation, 1)
	client := newAPIResourceTestClient(t, func(w http.ResponseWriter, r *http.Request) {
		if !recordAPIResourceRequest(t, requests, r) {
			return
		}
		w.WriteHeader(http.StatusNoContent)
	})
	resourceSchema := noteTagResourceSchema(t)
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, syntheticNoteTagModel()); diagnostics.HasError() {
		t.Fatalf("build delete state: %v", diagnostics)
	}

	response := resource.DeleteResponse{}
	(&noteTagResource{client: client}).Delete(ctx, resource.DeleteRequest{State: state}, &response)
	request := receiveAPIResourceRequest(requests)
	got := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: request, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		Request     *apiRequestObservation
		Diagnostics []apiDiagnosticObservation
	}{Request: &apiRequestObservation{Method: http.MethodDelete, Path: "/api/note-tags/synthetic_tag"}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("delete output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteTagResourceImportState(t *testing.T) {
	t.Parallel()

	ctx := context.Background()
	resourceSchema := noteTagResourceSchema(t)
	response := resource.ImportStateResponse{State: tfsdk.State{Schema: resourceSchema.Schema}}
	if diagnostics := response.State.Set(ctx, noteTagModel{
		Key:         types.StringNull(),
		DisplayName: types.StringNull(),
		Description: types.StringNull(),
	}); diagnostics.HasError() {
		t.Fatalf("build import state: %v", diagnostics)
	}

	(&noteTagResource{}).ImportState(ctx, resource.ImportStateRequest{ID: "synthetic_tag"}, &response)
	var imported noteTagModel
	response.Diagnostics.Append(response.State.Get(ctx, &imported)...)
	got := struct {
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{State: imported, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		State       noteTagModel
		Diagnostics []apiDiagnosticObservation
	}{State: noteTagModel{Key: types.StringValue("synthetic_tag"), DisplayName: types.StringNull(), Description: types.StringNull()}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestNoteTagResourceKeyRequiresReplacement(t *testing.T) {
	t.Parallel()

	resourceSchema := noteTagResourceSchema(t)
	attribute := resourceSchema.Schema.Attributes["key"].(schema.StringAttribute)
	ctx := context.Background()
	state := tfsdk.State{Schema: resourceSchema.Schema}
	if diagnostics := state.Set(ctx, syntheticNoteTagModel()); diagnostics.HasError() {
		t.Fatalf("build prior state: %v", diagnostics)
	}
	plan := tfsdk.Plan{Schema: resourceSchema.Schema}
	if diagnostics := plan.Set(ctx, noteTagModel{
		Key:         types.StringValue("renamed_synthetic_tag"),
		DisplayName: types.StringValue("Synthetic Tag"),
		Description: types.StringValue("synthetic description"),
	}); diagnostics.HasError() {
		t.Fatalf("build plan: %v", diagnostics)
	}
	request := planmodifier.StringRequest{
		Path:        path.Root("key"),
		State:       state,
		Plan:        plan,
		StateValue:  types.StringValue("synthetic_tag"),
		PlanValue:   types.StringValue("renamed_synthetic_tag"),
		ConfigValue: types.StringValue("renamed_synthetic_tag"),
	}
	response := planmodifier.StringResponse{PlanValue: request.PlanValue}
	attribute.PlanModifiers[0].PlanModifyString(ctx, request, &response)
	got := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []apiDiagnosticObservation
	}{PlanValue: response.PlanValue, RequiresReplace: response.RequiresReplace, Diagnostics: apiResourceDiagnosticsOutput(response.Diagnostics)}
	want := struct {
		PlanValue       types.String
		RequiresReplace bool
		Diagnostics     []apiDiagnosticObservation
	}{PlanValue: types.StringValue("renamed_synthetic_tag"), RequiresReplace: true}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("key plan modifier output mismatch: got=%#v want=%#v", got, want)
	}
}

func noteTagResourceSchema(t *testing.T) resource.SchemaResponse {
	t.Helper()
	var response resource.SchemaResponse
	(&noteTagResource{}).Schema(context.Background(), resource.SchemaRequest{}, &response)
	return response
}

func syntheticNoteTag() traderapigen.NoteTag {
	return traderapigen.NoteTag{
		Description: nullable.NewNullableWithValue("synthetic description"),
		DisplayName: "Synthetic Tag",
		Key:         "synthetic_tag",
	}
}

func syntheticNoteTagModel() noteTagModel {
	return modelFromNoteTag(syntheticNoteTag())
}
