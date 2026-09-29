package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/google/uuid"
	openapi_types "github.com/oapi-codegen/runtime/types"
)

func (c *Client) CreateCustomIndicator(ctx context.Context, strategyID *string, payload CreateCustomIndicatorRequest) (CustomIndicator, error) {
	if strategyID == nil {
		response, err := c.OpenAPI().CreateGlobalIndicatorWithResponse(ctx, payload)
		if err != nil {
			return CustomIndicator{}, fmt.Errorf("send create custom indicator request: %w", err)
		}
		if err := responseError(response.HTTPResponse, response.Body); err != nil {
			return CustomIndicator{}, err
		}
		if response.JSON201 == nil {
			return CustomIndicator{}, errors.New("backend returned HTTP 201 without a JSON response")
		}
		return *response.JSON201, nil
	}

	parsedStrategyID, err := parseStrategyID(*strategyID)
	if err != nil {
		return CustomIndicator{}, err
	}
	response, err := c.OpenAPI().CreateStrategyIndicatorWithResponse(ctx, parsedStrategyID, payload)
	if err != nil {
		return CustomIndicator{}, fmt.Errorf("send create custom indicator request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return CustomIndicator{}, err
	}
	if response.JSON201 == nil {
		return CustomIndicator{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetCustomIndicator(ctx context.Context, id string) (CustomIndicator, error) {
	indicatorID, err := parseCustomIndicatorID(id)
	if err != nil {
		return CustomIndicator{}, err
	}
	response, err := c.OpenAPI().GetIndicatorWithResponse(ctx, indicatorID)
	if err != nil {
		return CustomIndicator{}, fmt.Errorf("send get custom indicator request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return CustomIndicator{}, err
	}
	if response.JSON200 == nil {
		return CustomIndicator{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateCustomIndicator(ctx context.Context, id string, payload UpdateCustomIndicatorRequest) (CustomIndicator, error) {
	indicatorID, err := parseCustomIndicatorID(id)
	if err != nil {
		return CustomIndicator{}, err
	}
	response, err := c.OpenAPI().UpdateIndicatorWithResponse(ctx, indicatorID, payload)
	if err != nil {
		return CustomIndicator{}, fmt.Errorf("send update custom indicator request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return CustomIndicator{}, err
	}
	if response.JSON200 == nil {
		return CustomIndicator{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteCustomIndicator(ctx context.Context, id string) error {
	indicatorID, err := parseCustomIndicatorID(id)
	if err != nil {
		return err
	}
	response, err := c.OpenAPI().DeleteIndicatorWithResponse(ctx, indicatorID)
	if err != nil {
		return fmt.Errorf("send delete custom indicator request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}

func parseCustomIndicatorID(id string) (openapi_types.UUID, error) {
	if !idPattern.MatchString(id) {
		return uuid.Nil, errors.New("indicator id must be a UUID")
	}
	indicatorID, err := uuid.Parse(id)
	if err != nil {
		return uuid.Nil, fmt.Errorf("parse indicator id: %w", err)
	}
	return indicatorID, nil
}
