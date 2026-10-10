package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) ListPaperAccounts(ctx context.Context) ([]gen.PaperAccount, error) {
	response, err := c.api.ListPaperAccountsWithResponse(ctx)
	if err != nil {
		return nil, fmt.Errorf("send list paper accounts request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return nil, err
	}
	if response.JSON200 == nil {
		return nil, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) CreatePaperAccount(ctx context.Context, payload gen.CreatePaperAccountRequest) (gen.PaperAccount, error) {
	response, err := c.api.CreatePaperAccountWithResponse(ctx, payload)
	if err != nil {
		return gen.PaperAccount{}, fmt.Errorf("send create paper account request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.PaperAccount{}, err
	}
	if response.JSON201 == nil {
		return gen.PaperAccount{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}
