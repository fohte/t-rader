package traderapi

import (
	"context"
	"errors"
	"fmt"
)

func (c *Client) ListNoteKinds(ctx context.Context) ([]NoteKind, error) {
	response, err := c.api.ListNoteKindsWithResponse(ctx)
	if err != nil {
		return nil, fmt.Errorf("send list note kinds request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return nil, err
	}
	if response.JSON200 == nil {
		return nil, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) CreateNoteKind(ctx context.Context, payload CreateNoteKindRequest) (NoteKind, error) {
	response, err := c.api.CreateNoteKindWithResponse(ctx, payload)
	if err != nil {
		return NoteKind{}, fmt.Errorf("send create note kind request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return NoteKind{}, err
	}
	if response.JSON201 == nil {
		return NoteKind{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) UpdateNoteKind(ctx context.Context, key string, payload UpdateNoteKindRequest) (NoteKind, error) {
	response, err := c.api.UpdateNoteKindWithResponse(ctx, key, payload)
	if err != nil {
		return NoteKind{}, fmt.Errorf("send update note kind request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return NoteKind{}, err
	}
	if response.JSON200 == nil {
		return NoteKind{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteNoteKind(ctx context.Context, key string) error {
	response, err := c.api.DeleteNoteKindWithResponse(ctx, key)
	if err != nil {
		return fmt.Errorf("send delete note kind request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}
