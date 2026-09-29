package traderapi

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"time"

	"github.com/google/uuid"
)

var (
	ErrNotFound = errors.New("strategy not found")
	idPattern   = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
)

const strategiesPath = "/api/strategies"

type Client struct {
	*ClientWithResponses
}

func New(baseURL, clientID, clientSecret string) (*Client, error) {
	parsedURL, err := url.Parse(baseURL)
	if err != nil || parsedURL == nil || parsedURL.Host == "" || (parsedURL.Scheme != "http" && parsedURL.Scheme != "https") || parsedURL.User != nil || parsedURL.RawQuery != "" || parsedURL.Fragment != "" {
		return nil, errors.New("base_url must be an absolute HTTP or HTTPS URL without credentials, query, or fragment")
	}

	apiClient, err := NewClientWithResponses(
		parsedURL.String(),
		WithHTTPClient(&http.Client{Timeout: 30 * time.Second}),
		WithRequestEditorFn(func(_ context.Context, request *http.Request) error {
			request.Header.Set("Accept", "application/json")
			if clientID != "" {
				request.Header.Set("CF-Access-Client-Id", clientID)
				request.Header.Set("CF-Access-Client-Secret", clientSecret)
			}
			return nil
		}),
	)
	if err != nil {
		return nil, fmt.Errorf("create API client: %w", err)
	}

	return &Client{ClientWithResponses: apiClient}, nil
}

func (c *Client) CheckConnection(ctx context.Context) error {
	response, err := c.ListStrategiesWithResponse(ctx)
	if err != nil {
		return fmt.Errorf("send connection check: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}

func (c *Client) CreateStrategy(ctx context.Context, payload CreateStrategyRequest) (Strategy, error) {
	response, err := c.CreateStrategyWithResponse(ctx, payload)
	if err != nil {
		return Strategy{}, fmt.Errorf("send create strategy request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Strategy{}, err
	}
	if response.JSON201 == nil {
		return Strategy{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetStrategy(ctx context.Context, id string) (Strategy, error) {
	strategyID, err := parseStrategyID(id)
	if err != nil {
		return Strategy{}, err
	}
	response, err := c.GetStrategyWithResponse(ctx, strategyID)
	if err != nil {
		return Strategy{}, fmt.Errorf("send get strategy request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Strategy{}, err
	}
	if response.JSON200 == nil {
		return Strategy{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateStrategy(ctx context.Context, id string, payload UpdateStrategyRequest) (Strategy, error) {
	strategyID, err := parseStrategyID(id)
	if err != nil {
		return Strategy{}, err
	}
	response, err := c.UpdateStrategyWithResponse(ctx, strategyID, payload)
	if err != nil {
		return Strategy{}, fmt.Errorf("send update strategy request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return Strategy{}, err
	}
	if response.JSON200 == nil {
		return Strategy{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteStrategy(ctx context.Context, id string) error {
	strategyID, err := parseStrategyID(id)
	if err != nil {
		return err
	}
	response, err := c.DeleteStrategyWithResponse(ctx, strategyID)
	if err != nil {
		return fmt.Errorf("send delete strategy request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}

func parseStrategyID(id string) (uuid.UUID, error) {
	if !idPattern.MatchString(id) {
		return uuid.UUID{}, errors.New("strategy id must be a UUID")
	}
	strategyID, err := uuid.Parse(id)
	if err != nil {
		return uuid.UUID{}, fmt.Errorf("parse strategy id: %w", err)
	}
	return strategyID, nil
}

func responseError(response *http.Response, body []byte) error {
	if response == nil {
		return errors.New("backend response is missing")
	}
	if response.StatusCode >= http.StatusOK && response.StatusCode < http.StatusMultipleChoices {
		return nil
	}
	apiError := fmt.Errorf("backend returned HTTP %d: %s", response.StatusCode, strings.TrimSpace(string(body)))
	if response.StatusCode == http.StatusNotFound {
		return fmt.Errorf("%w: %w", ErrNotFound, apiError)
	}
	return apiError
}
