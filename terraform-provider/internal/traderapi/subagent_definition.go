package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) CreateSubagentDefinition(ctx context.Context, payload gen.CreateSubagentDefinitionRequest) (gen.SubagentDefinitionResponse, error) {
	response, err := c.api.CreateSubagentDefinitionWithResponse(ctx, payload)
	if err != nil {
		return gen.SubagentDefinitionResponse{}, fmt.Errorf("send create subagent definition request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.SubagentDefinitionResponse{}, err
	}
	if response.JSON201 == nil {
		return gen.SubagentDefinitionResponse{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetSubagentDefinition(ctx context.Context, key string) (gen.SubagentDefinitionResponse, error) {
	response, err := c.api.GetSubagentDefinitionWithResponse(ctx, key)
	if err != nil {
		return gen.SubagentDefinitionResponse{}, fmt.Errorf("send get subagent definition request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.SubagentDefinitionResponse{}, err
	}
	if response.JSON200 == nil {
		return gen.SubagentDefinitionResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateSubagentDefinition(ctx context.Context, key string, payload gen.UpdateSubagentDefinitionRequest) (gen.SubagentDefinitionResponse, error) {
	response, err := c.api.UpdateSubagentDefinitionWithResponse(ctx, key, payload)
	if err != nil {
		return gen.SubagentDefinitionResponse{}, fmt.Errorf("send update subagent definition request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.SubagentDefinitionResponse{}, err
	}
	if response.JSON200 == nil {
		return gen.SubagentDefinitionResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteSubagentDefinition(ctx context.Context, key string) error {
	response, err := c.api.DeleteSubagentDefinitionWithResponse(ctx, key)
	if err != nil {
		return fmt.Errorf("send delete subagent definition request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}
