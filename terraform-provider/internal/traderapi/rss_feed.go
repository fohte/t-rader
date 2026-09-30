package traderapi

import (
	"context"
	"errors"
	"fmt"

	"github.com/google/uuid"
)

func (c *Client) CreateRssFeed(ctx context.Context, payload CreateRssFeedRequest) (RssFeed, error) {
	response, err := c.api.CreateRssFeedWithResponse(ctx, payload)
	if err != nil {
		return RssFeed{}, fmt.Errorf("send create RSS feed request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return RssFeed{}, err
	}
	if response.JSON201 == nil {
		return RssFeed{}, errors.New("backend returned HTTP 201 without a JSON response")
	}
	return *response.JSON201, nil
}

func (c *Client) GetRssFeed(ctx context.Context, id string) (RssFeed, error) {
	feedID, err := parseRssFeedID(id)
	if err != nil {
		return RssFeed{}, err
	}
	response, err := c.api.GetRssFeedWithResponse(ctx, feedID)
	if err != nil {
		return RssFeed{}, fmt.Errorf("send get RSS feed request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return RssFeed{}, err
	}
	if response.JSON200 == nil {
		return RssFeed{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) UpdateRssFeed(ctx context.Context, id string, payload UpdateRssFeedRequest) (RssFeed, error) {
	feedID, err := parseRssFeedID(id)
	if err != nil {
		return RssFeed{}, err
	}
	response, err := c.api.UpdateRssFeedWithResponse(ctx, feedID, payload)
	if err != nil {
		return RssFeed{}, fmt.Errorf("send update RSS feed request: %w", err)
	}
	if err := responseError(response.HTTPResponse, response.Body); err != nil {
		return RssFeed{}, err
	}
	if response.JSON200 == nil {
		return RssFeed{}, errors.New("backend returned HTTP 200 without a JSON response")
	}
	return *response.JSON200, nil
}

func (c *Client) DeleteRssFeed(ctx context.Context, id string) error {
	feedID, err := parseRssFeedID(id)
	if err != nil {
		return err
	}
	response, err := c.api.DeleteRssFeedWithResponse(ctx, feedID)
	if err != nil {
		return fmt.Errorf("send delete RSS feed request: %w", err)
	}
	return responseError(response.HTTPResponse, response.Body)
}

func parseRssFeedID(id string) (uuid.UUID, error) {
	return parseResourceID("RSS feed", id)
}
