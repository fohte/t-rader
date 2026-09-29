package traderapi

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"regexp"
	"strings"
	"time"
)

const strategiesPath = "/api/strategies"

var (
	ErrNotFound = errors.New("strategy not found")
	idPattern   = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
)

type Strategy struct {
	ID          string  `json:"id"`
	Name        string  `json:"name"`
	Description *string `json:"description"`
	SortOrder   int32   `json:"sort_order"`
	CreatedAt   string  `json:"created_at"`
	UpdatedAt   string  `json:"updated_at"`
}

type CreateStrategyRequest struct {
	Name        string  `json:"name"`
	Description *string `json:"description,omitempty"`
	SortOrder   *int32  `json:"sort_order,omitempty"`
}

type UpdateStrategyRequest struct {
	Name        *string  `json:"name,omitempty"`
	Description **string `json:"description,omitempty"`
	SortOrder   *int32   `json:"sort_order,omitempty"`
}

type Client struct {
	baseURL      *url.URL
	httpClient   *http.Client
	clientID     string
	clientSecret string
}

func New(baseURL, clientID, clientSecret string) (*Client, error) {
	parsedURL, err := url.Parse(baseURL)
	if err != nil || parsedURL == nil || parsedURL.Host == "" || (parsedURL.Scheme != "http" && parsedURL.Scheme != "https") || parsedURL.User != nil || parsedURL.RawQuery != "" || parsedURL.Fragment != "" {
		return nil, errors.New("base_url must be an absolute HTTP or HTTPS URL without credentials, query, or fragment")
	}

	return &Client{
		baseURL:      parsedURL,
		httpClient:   &http.Client{Timeout: 30 * time.Second},
		clientID:     clientID,
		clientSecret: clientSecret,
	}, nil
}

func (c *Client) CheckConnection(ctx context.Context) error {
	return c.do(ctx, http.MethodGet, strategiesPath, nil, nil)
}

func (c *Client) CreateStrategy(ctx context.Context, payload CreateStrategyRequest) (Strategy, error) {
	var strategy Strategy
	if err := c.do(ctx, http.MethodPost, strategiesPath, payload, &strategy); err != nil {
		return Strategy{}, err
	}
	return strategy, nil
}

func (c *Client) GetStrategy(ctx context.Context, id string) (Strategy, error) {
	requestPath, err := strategyPath(id)
	if err != nil {
		return Strategy{}, err
	}
	var strategy Strategy
	if err := c.do(ctx, http.MethodGet, requestPath, nil, &strategy); err != nil {
		return Strategy{}, err
	}
	return strategy, nil
}

func (c *Client) UpdateStrategy(ctx context.Context, id string, payload UpdateStrategyRequest) (Strategy, error) {
	requestPath, err := strategyPath(id)
	if err != nil {
		return Strategy{}, err
	}
	var strategy Strategy
	if err := c.do(ctx, http.MethodPatch, requestPath, payload, &strategy); err != nil {
		return Strategy{}, err
	}
	return strategy, nil
}

func (c *Client) DeleteStrategy(ctx context.Context, id string) error {
	requestPath, err := strategyPath(id)
	if err != nil {
		return err
	}
	return c.do(ctx, http.MethodDelete, requestPath, nil, nil)
}

func (c *Client) do(ctx context.Context, method, requestPath string, payload, result any) (resultErr error) {
	var requestBody bytes.Buffer
	if payload != nil {
		if err := json.NewEncoder(&requestBody).Encode(payload); err != nil {
			return fmt.Errorf("encode request: %w", err)
		}
	}

	targetURL := *c.baseURL
	targetURL.Path = strings.TrimRight(targetURL.Path, "/") + requestPath
	targetURL.RawPath = ""
	var body io.Reader
	if payload != nil {
		body = &requestBody
	}
	request, err := http.NewRequestWithContext(ctx, method, targetURL.String(), body)
	if err != nil {
		return fmt.Errorf("create request: %w", err)
	}
	request.Header.Set("Accept", "application/json")
	if payload != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	if c.clientID != "" {
		request.Header.Set("CF-Access-Client-Id", c.clientID)
		request.Header.Set("CF-Access-Client-Secret", c.clientSecret)
	}

	response, err := c.httpClient.Do(request)
	if err != nil {
		return fmt.Errorf("send request: %w", err)
	}
	defer func() {
		if err := response.Body.Close(); err != nil && resultErr == nil {
			resultErr = fmt.Errorf("close response body: %w", err)
		}
	}()

	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		responseBody, readErr := io.ReadAll(io.LimitReader(response.Body, 64*1024))
		if readErr != nil {
			return fmt.Errorf("read error response: %w", readErr)
		}
		apiError := fmt.Errorf("backend returned HTTP %d: %s", response.StatusCode, strings.TrimSpace(string(responseBody)))
		if response.StatusCode == http.StatusNotFound {
			return fmt.Errorf("%w: %w", ErrNotFound, apiError)
		}
		return apiError
	}

	if result == nil {
		_, _ = io.Copy(io.Discard, response.Body)
		return nil
	}
	if err := json.NewDecoder(response.Body).Decode(result); err != nil {
		return fmt.Errorf("decode response: %w", err)
	}
	return nil
}

func strategyPath(id string) (string, error) {
	if !idPattern.MatchString(id) {
		return "", errors.New("strategy id must be a UUID")
	}
	return strategiesPath + "/" + id, nil
}
