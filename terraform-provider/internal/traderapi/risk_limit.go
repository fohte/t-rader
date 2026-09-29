package traderapi

import (
	"context"
	"errors"
	"fmt"
)

func (c *Client) GetRiskLimit(ctx context.Context) (AccountRiskPolicyResponse, error) {
	response, err := c.api.GetAccountRiskPolicyWithResponse(ctx)
	if err != nil {
		return AccountRiskPolicyResponse{}, fmt.Errorf("send get risk limit request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return AccountRiskPolicyResponse{}, err
	}
	if response.JSON200 == nil {
		return AccountRiskPolicyResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) PutRiskLimit(ctx context.Context, payload PutAccountRiskPolicyRequest) (AccountRiskPolicyResponse, error) {
	response, err := c.api.PutAccountRiskPolicyWithResponse(ctx, payload)
	if err != nil {
		return AccountRiskPolicyResponse{}, fmt.Errorf("send put risk limit request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return AccountRiskPolicyResponse{}, err
	}
	if response.JSON200 == nil {
		return AccountRiskPolicyResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}
