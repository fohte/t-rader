package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) ListStrategyEarningsTargets(ctx context.Context, strategyID string) ([]gen.StrategyEarningsTargetResponse, error) {
	parsedStrategyID, err := parseStrategyID(strategyID)
	if err != nil {
		return nil, err
	}
	response, err := c.api.ListStrategyEarningsTargetsWithResponse(ctx, parsedStrategyID)
	if err != nil {
		return nil, fmt.Errorf("send list strategy earnings targets request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return nil, err
	}
	if response.JSON200 == nil {
		return nil, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) AddStrategyEarningsTarget(ctx context.Context, strategyID, refKind, refID string) error {
	parsedStrategyID, err := parseStrategyID(strategyID)
	if err != nil {
		return err
	}
	response, err := c.api.AddStrategyEarningsTargetWithResponse(ctx, parsedStrategyID, gen.AddStrategyEarningsTargetRequest{
		RefId:   refID,
		RefKind: refKind,
	})
	if err != nil {
		return fmt.Errorf("send add strategy earnings target request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return err
	}
	if response.JSON200 == nil {
		return errors.New("backend returned HTTP 200 without a JSON response")
	}
	return nil
}

func (c *Client) RemoveStrategyEarningsTarget(ctx context.Context, strategyID, refKind, refID string) error {
	parsedStrategyID, err := parseStrategyID(strategyID)
	if err != nil {
		return err
	}
	response, err := c.api.RemoveStrategyEarningsTargetWithResponse(ctx, parsedStrategyID, &gen.RemoveStrategyEarningsTargetParams{
		RefId:   refID,
		RefKind: refKind,
	})
	if err != nil {
		return fmt.Errorf("send remove strategy earnings target request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return err
	}
	if response.JSON200 == nil {
		return errors.New("backend returned HTTP 200 without a JSON response")
	}
	return nil
}
