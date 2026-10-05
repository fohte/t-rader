package traderapi

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"

	"github.com/google/uuid"
	"github.com/oapi-codegen/nullable"
)

func (c *Client) CreateStrategyTrigger(ctx context.Context, strategyID string, payload gen.CreateTriggerRequest) (gen.Trigger, error) {
	parsedStrategyID, err := parseStrategyID(strategyID)
	if err != nil {
		return gen.Trigger{}, err
	}
	response, err := c.api.CreateStrategyTriggerWithResponse(ctx, parsedStrategyID, payload)
	if err != nil {
		return gen.Trigger{}, fmt.Errorf("send create strategy trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Trigger{}, err
	}
	if response.JSON201 == nil {
		return gen.Trigger{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return triggerWithExactEventMatch(*response.JSON201, response.Body)
}

func (c *Client) GetTrigger(ctx context.Context, id string) (gen.Trigger, error) {
	triggerID, err := parseTriggerID(id)
	if err != nil {
		return gen.Trigger{}, err
	}
	response, err := c.api.GetTriggerWithResponse(ctx, triggerID)
	if err != nil {
		return gen.Trigger{}, fmt.Errorf("send get trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Trigger{}, err
	}
	if response.JSON200 == nil {
		return gen.Trigger{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return triggerWithExactEventMatch(*response.JSON200, response.Body)
}

func (c *Client) UpdateTrigger(ctx context.Context, id string, payload gen.UpdateTriggerRequest) (gen.Trigger, error) {
	triggerID, err := parseTriggerID(id)
	if err != nil {
		return gen.Trigger{}, err
	}
	response, err := c.api.UpdateTriggerWithResponse(ctx, triggerID, payload)
	if err != nil {
		return gen.Trigger{}, fmt.Errorf("send update trigger request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.Trigger{}, err
	}
	if response.JSON200 == nil {
		return gen.Trigger{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return triggerWithExactEventMatch(*response.JSON200, response.Body)
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

func triggerWithExactEventMatch(trigger gen.Trigger, body []byte) (gen.Trigger, error) {
	var response struct {
		EventMatch json.RawMessage `json:"event_match"`
	}
	if err := json.Unmarshal(body, &response); err != nil {
		return gen.Trigger{}, fmt.Errorf("decode trigger event_match: %w", err)
	}
	if response.EventMatch == nil {
		trigger.EventMatch = nullable.Nullable[map[string]interface{}]{}
		return trigger, nil
	}
	if bytes.Equal(bytes.TrimSpace(response.EventMatch), []byte("null")) {
		trigger.EventMatch = nullable.NewNullNullable[map[string]interface{}]()
		return trigger, nil
	}

	decoder := json.NewDecoder(bytes.NewReader(response.EventMatch))
	decoder.UseNumber()
	var eventMatch map[string]interface{}
	if err := decoder.Decode(&eventMatch); err != nil {
		return gen.Trigger{}, fmt.Errorf("decode trigger event_match: %w", err)
	}
	trigger.EventMatch = nullable.NewNullableWithValue(eventMatch)
	return trigger, nil
}
