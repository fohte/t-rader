package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/google/uuid"
)

func (c *Client) CreateStrategyTrigger(ctx context.Context, strategyID string, payload CreateTriggerRequest) (Trigger, error) {
	parsedStrategyID, err := parseStrategyID(strategyID)
	if err != nil {
		return Trigger{}, err
	}
	response, err := c.api.CreateStrategyTriggerWithResponse(ctx, parsedStrategyID, payload)
	if err != nil {
		return Trigger{}, fmt.Errorf("send create strategy trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Trigger{}, err
	}
	if response.JSON201 == nil {
		return Trigger{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetTrigger(ctx context.Context, id string) (Trigger, error) {
	triggerID, err := parseTriggerID(id)
	if err != nil {
		return Trigger{}, err
	}
	response, err := c.api.GetTriggerWithResponse(ctx, triggerID)
	if err != nil {
		return Trigger{}, fmt.Errorf("send get trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Trigger{}, err
	}
	if response.JSON200 == nil {
		return Trigger{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateTrigger(ctx context.Context, id string, payload UpdateTriggerRequest) (Trigger, error) {
	triggerID, err := parseTriggerID(id)
	if err != nil {
		return Trigger{}, err
	}
	response, err := c.api.UpdateTriggerWithResponse(ctx, triggerID, payload)
	if err != nil {
		return Trigger{}, fmt.Errorf("send update trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Trigger{}, err
	}
	if response.JSON200 == nil {
		return Trigger{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteTrigger(ctx context.Context, id string) error {
	triggerID, err := parseTriggerID(id)
	if err != nil {
		return err
	}
	response, err := c.api.DeleteTriggerWithResponse(ctx, triggerID)
	if err != nil {
		return fmt.Errorf("send delete trigger request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}

func parseTriggerID(id string) (uuid.UUID, error) {
	if !idPattern.MatchString(id) {
		return uuid.UUID{}, errors.New("trigger id must be a UUID")
	}
	triggerID, err := uuid.Parse(id)
	if err != nil {
		return uuid.UUID{}, fmt.Errorf("parse trigger id: %w", err)
	}
	return triggerID, nil
}
