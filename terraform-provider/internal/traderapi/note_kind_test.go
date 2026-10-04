package traderapi

import (
	"context"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
	"net/http"
	"reflect"
	"testing"

	"github.com/oapi-codegen/nullable"
)

func TestClientListNoteKinds(t *testing.T) {
	t.Parallel()

	wantKind := testNoteKind()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, []gen.NoteKind{wantKind})
	})

	result, err := client.ListNoteKinds(context.Background())
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  []gen.NoteKind
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  []gen.NoteKind
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/note-kinds",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: []gen.NoteKind{wantKind},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("list note kinds output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientCreateNoteKind(t *testing.T) {
	t.Parallel()

	created := testNoteKind()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusCreated, created)
	})

	result, err := client.CreateNoteKind(context.Background(), gen.CreateNoteKindRequest{
		Description:      nullable.NewNullableWithValue("synthetic description"),
		DisplayName:      "Synthetic Kind",
		Key:              "synthetic_kind",
		RequiresApproval: true,
		SortOrder:        nullable.NewNullableWithValue(int32(3)),
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.NoteKind
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.NoteKind
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPost,
			Path:         "/api/note-kinds",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"description":"synthetic description","display_name":"Synthetic Kind","key":"synthetic_kind","requires_approval":true,"sort_order":3}`,
		},
		Result: created,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("create note kind output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateNoteKind(t *testing.T) {
	t.Parallel()

	updated := testNoteKind()
	updated.Description = nullable.NewNullNullable[string]()
	updated.DisplayName = "Updated Synthetic Kind"
	updated.RequiresApproval = false
	updated.SortOrder = 5
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, updated)
	})

	result, err := client.UpdateNoteKind(context.Background(), "synthetic_kind", gen.UpdateNoteKindRequest{
		Description:      nullable.NewNullNullable[string](),
		DisplayName:      nullable.NewNullableWithValue("Updated Synthetic Kind"),
		RequiresApproval: nullable.NewNullableWithValue(false),
		SortOrder:        nullable.NewNullableWithValue(int32(5)),
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.NoteKind
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.NoteKind
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPatch,
			Path:         "/api/note-kinds/synthetic_kind",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"description":null,"display_name":"Updated Synthetic Kind","requires_approval":false,"sort_order":5}`,
		},
		Result: updated,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("update note kind output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteNoteKind(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteNoteKind(context.Background(), "synthetic_kind")
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
			Path:         "/api/note-kinds/synthetic_kind",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete note kind output mismatch: got=%#v want=%#v", got, want)
	}
}

func testNoteKind() gen.NoteKind {
	return gen.NoteKind{
		Description:      nullable.NewNullableWithValue("synthetic description"),
		DisplayName:      "Synthetic Kind",
		Key:              "synthetic_kind",
		RequiresApproval: true,
		SortOrder:        3,
	}
}
