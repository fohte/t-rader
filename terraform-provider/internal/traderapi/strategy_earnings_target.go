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

func (c *Client) ResolveRef(ctx context.Context, refKind, refID string) (gen.RefResolution, error) {
	response, err := c.api.ResolveRefsWithResponse(ctx, &gen.ResolveRefsParams{Link: refKind + ":" + refID})
	if err != nil {
		return gen.RefResolution{}, fmt.Errorf("send resolve reference request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.RefResolution{}, err
	}
	if response.JSON200 == nil {
		return gen.RefResolution{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	if len(*response.JSON200) != 1 {
		return gen.RefResolution{}, fmt.Errorf("backend returned %d references for one resolve request", len(*response.JSON200))
	}
	return (*response.JSON200)[0], nil
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
