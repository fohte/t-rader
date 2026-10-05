package traderapi

import (
	"context"
	"errors"
	"fmt"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) CreateGroupAxis(ctx context.Context, payload gen.CreateGroupAxisRequest) (gen.GroupAxis, error) {
	response, err := c.api.CreateGroupAxisWithResponse(ctx, payload)
	if err != nil {
		return gen.GroupAxis{}, fmt.Errorf("send create group axis request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.GroupAxis{}, err
	}
	if response.JSON201 == nil {
		return gen.GroupAxis{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetGroupAxis(ctx context.Context, key string) (gen.GroupAxis, error) {
	response, err := c.api.GetGroupAxisWithResponse(ctx, key)
	if err != nil {
		return gen.GroupAxis{}, fmt.Errorf("send get group axis request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.GroupAxis{}, err
	}
	if response.JSON200 == nil {
		return gen.GroupAxis{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateGroupAxis(ctx context.Context, key string, payload gen.UpdateGroupAxisRequest) (gen.GroupAxis, error) {
	response, err := c.api.UpdateGroupAxisWithResponse(ctx, key, payload)
	if err != nil {
		return gen.GroupAxis{}, fmt.Errorf("send update group axis request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.GroupAxis{}, err
	}
	if response.JSON200 == nil {
		return gen.GroupAxis{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteGroupAxis(ctx context.Context, key string) error {
	response, err := c.api.DeleteGroupAxisWithResponse(ctx, key)
	if err != nil {
		return fmt.Errorf("send delete group axis request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}
