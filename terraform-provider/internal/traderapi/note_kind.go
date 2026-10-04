package traderapi

import (
	"context"
	"errors"
	"fmt"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) ListNoteKinds(ctx context.Context) ([]gen.NoteKind, error) {
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

func (c *Client) CreateNoteKind(ctx context.Context, payload gen.CreateNoteKindRequest) (gen.NoteKind, error) {
	response, err := c.api.CreateNoteKindWithResponse(ctx, payload)
	if err != nil {
		return gen.NoteKind{}, fmt.Errorf("send create note kind request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.NoteKind{}, err
	}
	if response.JSON201 == nil {
		return gen.NoteKind{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) UpdateNoteKind(ctx context.Context, key string, payload gen.UpdateNoteKindRequest) (gen.NoteKind, error) {
	response, err := c.api.UpdateNoteKindWithResponse(ctx, key, payload)
	if err != nil {
		return gen.NoteKind{}, fmt.Errorf("send update note kind request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.NoteKind{}, err
	}
	if response.JSON200 == nil {
		return gen.NoteKind{}, errors.New("backend returned HTTP 200 without a JSON response")
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
