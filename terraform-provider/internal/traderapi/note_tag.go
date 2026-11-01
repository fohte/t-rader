package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) ListNoteTags(ctx context.Context) ([]gen.NoteTag, error) {
	response, err := c.api.ListNoteTagsWithResponse(ctx)
	if err != nil {
		return nil, fmt.Errorf("send list note tags request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return nil, err
	}
	if response.JSON200 == nil {
		return nil, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) CreateNoteTag(ctx context.Context, payload gen.CreateNoteTagRequest) (gen.NoteTag, error) {
	response, err := c.api.CreateNoteTagWithResponse(ctx, payload)
	if err != nil {
		return gen.NoteTag{}, fmt.Errorf("send create note tag request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.NoteTag{}, err
	}
	if response.JSON201 == nil {
		return gen.NoteTag{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) UpdateNoteTag(ctx context.Context, key string, payload gen.UpdateNoteTagRequest) (gen.NoteTag, error) {
	response, err := c.api.UpdateNoteTagWithResponse(ctx, key, payload)
	if err != nil {
		return gen.NoteTag{}, fmt.Errorf("send update note tag request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.NoteTag{}, err
	}
	if response.JSON200 == nil {
		return gen.NoteTag{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteNoteTag(ctx context.Context, key string) error {
	response, err := c.api.DeleteNoteTagWithResponse(ctx, key)
	if err != nil {
		return fmt.Errorf("send delete note tag request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}
