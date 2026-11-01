package traderapi

import (
	"context"
	"net/http"
	"reflect"
	"testing"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
	"github.com/oapi-codegen/nullable"
)

func TestClientListNoteTags(t *testing.T) {
	t.Parallel()

	wantTag := testNoteTag()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, []gen.NoteTag{wantTag})
	})

	result, err := client.ListNoteTags(context.Background())
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  []gen.NoteTag
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  []gen.NoteTag
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/note-tags",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: []gen.NoteTag{wantTag},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("list note tags output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientCreateNoteTag(t *testing.T) {
	t.Parallel()

	created := testNoteTag()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusCreated, created)
	})

	result, err := client.CreateNoteTag(context.Background(), gen.CreateNoteTagRequest{
		Description: nullable.NewNullableWithValue("synthetic description"),
		DisplayName: "Synthetic Tag",
		Key:         "synthetic_tag",
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.NoteTag
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.NoteTag
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPost,
			Path:         "/api/note-tags",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"description":"synthetic description","display_name":"Synthetic Tag","key":"synthetic_tag"}`,
		},
		Result: created,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("create note tag output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateNoteTag(t *testing.T) {
	t.Parallel()

	updated := testNoteTag()
	updated.Description = nullable.NewNullNullable[string]()
	updated.DisplayName = "Updated Synthetic Tag"
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, updated)
	})

	result, err := client.UpdateNoteTag(context.Background(), "synthetic_tag", gen.UpdateNoteTagRequest{
		Description: nullable.NewNullNullable[string](),
		DisplayName: nullable.NewNullableWithValue("Updated Synthetic Tag"),
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  gen.NoteTag
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  gen.NoteTag
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPatch,
			Path:         "/api/note-tags/synthetic_tag",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"description":null,"display_name":"Updated Synthetic Tag"}`,
		},
		Result: updated,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("update note tag output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteNoteTag(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteNoteTag(context.Background(), "synthetic_tag")
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Error   string
	}{request, errorMessage(err)}, struct {
		Request observedRequest
		Error   string
	}{Request: observedRequest{
		Method:       http.MethodDelete,
		Path:         "/api/note-tags/synthetic_tag",
		ClientID:     testClientID,
		ClientSecret: testSecret,
		Accept:       "application/json",
	}}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete note tag output mismatch: got=%#v want=%#v", got, want)
	}
}

func testNoteTag() gen.NoteTag {
	return gen.NoteTag{
		Description: nullable.NewNullableWithValue("synthetic description"),
		DisplayName: "Synthetic Tag",
		Key:         "synthetic_tag",
	}
}
