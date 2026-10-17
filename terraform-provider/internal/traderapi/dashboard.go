package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) CreateDashboard(ctx context.Context, payload gen.CreateDashboardRequest) (gen.Dashboard, error) {
	response, err := c.OpenAPI().CreateDashboardWithResponse(ctx, payload)
	if err != nil {
		return gen.Dashboard{}, fmt.Errorf("send create dashboard request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Dashboard{}, err
	}
	if response.JSON201 == nil {
		return gen.Dashboard{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetDashboard(ctx context.Context, key string) (gen.Dashboard, error) {
	response, err := c.OpenAPI().GetDashboardWithResponse(ctx, key)
	if err != nil {
		return gen.Dashboard{}, fmt.Errorf("send get dashboard request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Dashboard{}, err
	}
	if response.JSON200 == nil {
		return gen.Dashboard{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateDashboard(ctx context.Context, key string, payload gen.UpdateDashboardRequest) (gen.Dashboard, error) {
	response, err := c.OpenAPI().UpdateDashboardWithResponse(ctx, key, payload)
	if err != nil {
		return gen.Dashboard{}, fmt.Errorf("send update dashboard request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Dashboard{}, err
	}
	if response.JSON200 == nil {
		return gen.Dashboard{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteDashboard(ctx context.Context, key string) error {
	response, err := c.OpenAPI().DeleteDashboardWithResponse(ctx, key)
	if err != nil {
		return fmt.Errorf("send delete dashboard request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}
