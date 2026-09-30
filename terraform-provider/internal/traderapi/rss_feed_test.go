package traderapi

import (
	"context"
	"errors"
	"net/http"
	"reflect"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/oapi-codegen/nullable"
)

const testRssFeedID = "00000000-0000-4000-8000-000000000002"

func TestClientCreateRssFeed(t *testing.T) {
	t.Parallel()

	created := testRssFeed()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusCreated, created)
	})

	result, err := client.CreateRssFeed(context.Background(), CreateRssFeedRequest{
		DisplayName: "Synthetic Feed",
		Enabled:     nullable.NewNullableWithValue(true),
		Source:      "synthetic_feed",
		Url:         "https://example.invalid/feed.xml",
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPost,
			Path:         "/api/rss-feeds",
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"display_name":"Synthetic Feed","enabled":true,"source":"synthetic_feed","url":"https://example.invalid/feed.xml"}`,
		},
		Result: created,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("create RSS feed output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientGetRssFeed(t *testing.T) {
	t.Parallel()

	feed := testRssFeed()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, feed)
	})

	result, err := client.GetRssFeed(context.Background(), testRssFeedID)
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodGet,
			Path:         "/api/rss-feeds/" + testRssFeedID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Result: feed,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("get RSS feed output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientUpdateRssFeed(t *testing.T) {
	t.Parallel()

	updated := testRssFeedWithUpdate()
	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		writeJSON(t, w, http.StatusOK, updated)
	})

	result, err := client.UpdateRssFeed(context.Background(), testRssFeedID, UpdateRssFeedRequest{
		DisplayName: nullable.NewNullableWithValue("Updated Synthetic Feed"),
		Enabled:     nullable.NewNullNullable[bool](),
		Url:         nullable.NewNullableWithValue("https://example.invalid/updated.xml"),
	})
	request := <-requests
	if got, want := struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{request, result, errorMessage(err)}, struct {
		Request observedRequest
		Result  RssFeed
		Error   string
	}{
		Request: observedRequest{
			Method:       http.MethodPatch,
			Path:         "/api/rss-feeds/" + testRssFeedID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
			ContentType:  "application/json",
			Body:         `{"display_name":"Updated Synthetic Feed","enabled":null,"url":"https://example.invalid/updated.xml"}`,
		},
		Result: updated,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("update RSS feed output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientDeleteRssFeed(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		w.WriteHeader(http.StatusNoContent)
	})

	err := client.DeleteRssFeed(context.Background(), testRssFeedID)
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
			Path:         "/api/rss-feeds/" + testRssFeedID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("delete RSS feed output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientReturnsNotFoundForMissingRssFeed(t *testing.T) {
	t.Parallel()

	client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
		http.Error(w, "synthetic missing RSS feed", http.StatusNotFound)
	})
	_, err := client.GetRssFeed(context.Background(), testRssFeedID)
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
			Path:         "/api/rss-feeds/" + testRssFeedID,
			ClientID:     testClientID,
			ClientSecret: testSecret,
			Accept:       "application/json",
		},
		Error:    "resource not found: backend returned HTTP 404: synthetic missing RSS feed",
		NotFound: true,
	}; !reflect.DeepEqual(got, want) {
		t.Fatalf("missing RSS feed output mismatch: got=%#v want=%#v", got, want)
	}
}

func TestClientRejectsNonUUIDRssFeedID(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		call func(*Client) error
	}{
		{
			name: "get",
			call: func(client *Client) error {
				_, err := client.GetRssFeed(context.Background(), "not-a-uuid")
				return err
			},
		},
		{
			name: "update",
			call: func(client *Client) error {
				_, err := client.UpdateRssFeed(context.Background(), "not-a-uuid", UpdateRssFeedRequest{})
				return err
			},
		},
		{
			name: "delete",
			call: func(client *Client) error {
				return client.DeleteRssFeed(context.Background(), "not-a-uuid")
			},
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			client, requests := newTestClient(t, func(w http.ResponseWriter, _ *http.Request, _ []byte) {
				t.Errorf("unexpected request for invalid RSS feed ID")
				w.WriteHeader(http.StatusOK)
			})
			err := testCase.call(client)
			requestReceived := false
			select {
			case <-requests:
				requestReceived = true
			default:
			}
			if got, want := struct {
				Error           string
				RequestReceived bool
			}{errorMessage(err), requestReceived}, struct {
				Error           string
				RequestReceived bool
			}{"RSS feed id must be a UUID", false}; !reflect.DeepEqual(got, want) {
				t.Fatalf("invalid RSS feed ID output mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}

func testRssFeed() RssFeed {
	return RssFeed{
		CreatedAt:   time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		DisplayName: "Synthetic Feed",
		Enabled:     true,
		Id:          uuid.MustParse(testRssFeedID),
		Source:      "synthetic_feed",
		UpdatedAt:   time.Date(2026, time.January, 1, 0, 0, 0, 0, time.UTC),
		Url:         "https://example.invalid/feed.xml",
	}
}

func testRssFeedWithUpdate() RssFeed {
	feed := testRssFeed()
	feed.DisplayName = "Updated Synthetic Feed"
	feed.UpdatedAt = time.Date(2026, time.January, 2, 0, 0, 0, 0, time.UTC)
	feed.Url = "https://example.invalid/updated.xml"
	return feed
}
