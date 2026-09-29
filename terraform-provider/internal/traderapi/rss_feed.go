package traderapi

import (
	"context"
	"errors"
	"net/http"
)

const rssFeedsPath = "/api/rss-feeds"

type RssFeed struct {
	ID          string `json:"id"`
	Source      string `json:"source"`
	DisplayName string `json:"display_name"`
	URL         string `json:"url"`
	Enabled     bool   `json:"enabled"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

type CreateRssFeedRequest struct {
	Source      string `json:"source"`
	DisplayName string `json:"display_name"`
	URL         string `json:"url"`
	Enabled     *bool  `json:"enabled,omitempty"`
}

type UpdateRssFeedRequest struct {
	DisplayName *string `json:"display_name,omitempty"`
	URL         *string `json:"url,omitempty"`
	Enabled     *bool   `json:"enabled,omitempty"`
}

func (c *Client) CreateRssFeed(ctx context.Context, payload CreateRssFeedRequest) (RssFeed, error) {
	var feed RssFeed
	if err := c.do(ctx, http.MethodPost, rssFeedsPath, payload, &feed); err != nil {
		return RssFeed{}, err
	}
	return feed, nil
}

func (c *Client) GetRssFeed(ctx context.Context, id string) (RssFeed, error) {
	requestPath, err := rssFeedPath(id)
	if err != nil {
		return RssFeed{}, err
	}
	var feed RssFeed
	if err := c.do(ctx, http.MethodGet, requestPath, nil, &feed); err != nil {
		return RssFeed{}, err
	}
	return feed, nil
}

func (c *Client) UpdateRssFeed(ctx context.Context, id string, payload UpdateRssFeedRequest) (RssFeed, error) {
	requestPath, err := rssFeedPath(id)
	if err != nil {
		return RssFeed{}, err
	}
	var feed RssFeed
	if err := c.do(ctx, http.MethodPatch, requestPath, payload, &feed); err != nil {
		return RssFeed{}, err
	}
	return feed, nil
}

func (c *Client) DeleteRssFeed(ctx context.Context, id string) error {
	requestPath, err := rssFeedPath(id)
	if err != nil {
		return err
	}
	return c.do(ctx, http.MethodDelete, requestPath, nil, nil)
}

func rssFeedPath(id string) (string, error) {
	if !idPattern.MatchString(id) {
		return "", errors.New("rss feed id must be a UUID")
	}
	return rssFeedsPath + "/" + id, nil
}
