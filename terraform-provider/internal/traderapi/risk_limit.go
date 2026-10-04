package traderapi

import (
	"context"
	"errors"
	"fmt"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
)

func (c *Client) GetRiskLimit(ctx context.Context) (gen.AccountRiskPolicyResponse, error) {
	response, err := c.api.GetAccountRiskPolicyWithResponse(ctx)
	if err != nil {
		return gen.AccountRiskPolicyResponse{}, fmt.Errorf("send get risk limit request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.AccountRiskPolicyResponse{}, err
	}
	if response.JSON200 == nil {
		return gen.AccountRiskPolicyResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) PutRiskLimit(ctx context.Context, payload gen.PutAccountRiskPolicyRequest) (gen.AccountRiskPolicyResponse, error) {
	response, err := c.api.PutAccountRiskPolicyWithResponse(ctx, payload)
	if err != nil {
		return gen.AccountRiskPolicyResponse{}, fmt.Errorf("send put risk limit request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return gen.AccountRiskPolicyResponse{}, err
	}
	if response.JSON200 == nil {
		return gen.AccountRiskPolicyResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}
