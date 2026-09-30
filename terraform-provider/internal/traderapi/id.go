package traderapi

import (
	"fmt"

	"github.com/google/uuid"
)

func parseResourceID(resource, id string) (uuid.UUID, error) {
	if !idPattern.MatchString(id) {
		return uuid.UUID{}, fmt.Errorf("%s id must be a UUID", resource)
	}
	parsedID, err := uuid.Parse(id)
	if err != nil {
		return uuid.UUID{}, fmt.Errorf("parse %s id: %w", resource, err)
	}
	return parsedID, nil
}
