package traderapi

import (
	"context"
	"errors"
	"fmt"
	"net/http"
)

var ErrAgentConfigNotFound = errors.New("agent config not found")

func (c *Client) CreateAgentConfig(ctx context.Context, purpose string) error {
	response, err := c.api.CreateAgentConfigWithResponse(ctx, CreateAgentConfigJSONRequestBody{Purpose: purpose})
	if err != nil {
		return fmt.Errorf("send create agent config request: %w", err)
	}
	if err := agentConfigResponseError(response.HTTPResponse, response.Body); err != nil {
		return err
	}
	if response.JSON201 == nil {
		return errors.New("backend returned HTTP 201 without a JSON response")
	}
	return nil
}

func (c *Client) GetAgentConfig(ctx context.Context, purpose string) (AgentConfigResponse, error) {
	response, err := c.api.AgentConfigGetAgentConfigBundleWithResponse(ctx, purpose)
	if err != nil {
		return AgentConfigResponse{}, fmt.Errorf("send get agent config request: %w", err)
	}
	if err := agentConfigResponseError(response.HTTPResponse, response.Body); err != nil {
		return AgentConfigResponse{}, err
	}
	if response.JSON200 == nil {
		return AgentConfigResponse{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateAgentConfig(ctx context.Context, purpose, agentsMd string, skills map[string]string, agentGraph string) error {
	skillsResponse, err := c.api.AgentConfigPutSkillsWithResponse(ctx, purpose, AgentConfigPutSkillsJSONRequestBody{
		Skills: skills,
	})
	if err != nil {
		return fmt.Errorf("send update agent skills request: %w", err)
	}
	if err := agentConfigResponseError(skillsResponse.HTTPResponse, skillsResponse.Body); err != nil {
		return err
	}

	agentsMDResponse, err := c.api.AgentConfigPutAgentsMdWithResponse(ctx, purpose, AgentConfigPutAgentsMdJSONRequestBody{
		Content: agentsMd,
	})
	if err != nil {
		return fmt.Errorf("send update agents md request: %w", err)
	}
	if err := agentConfigResponseError(agentsMDResponse.HTTPResponse, agentsMDResponse.Body); err != nil {
		return err
	}

	graphResponse, err := c.api.AgentConfigPutAgentGraphWithResponse(ctx, purpose, AgentConfigPutAgentGraphJSONRequestBody{
		Content: agentGraph,
	})
	if err != nil {
		return fmt.Errorf("send update agent graph request: %w", err)
	}
	return agentConfigResponseError(graphResponse.HTTPResponse, graphResponse.Body)
}

func (c *Client) DeleteAgentConfig(ctx context.Context, purpose string) error {
	response, err := c.api.DeleteAgentConfigWithResponse(ctx, purpose)
	if err != nil {
		return fmt.Errorf("send delete agent config request: %w", err)
	}
	return agentConfigResponseError(response.HTTPResponse, response.Body)
}

func agentConfigResponseError(response *http.Response, body []byte) error {
	err := responseError(response, body)
	if errors.Is(err, ErrNotFound) {
		return fmt.Errorf("%w: %w", ErrAgentConfigNotFound, err)
	}
	return err
}
