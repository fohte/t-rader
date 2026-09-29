package provider

import (
	"context"
	"encoding/json"
	"reflect"
	"testing"
)

func TestCustomIndicatorJSONRoundTrip(t *testing.T) {
	t.Parallel()

	remote := map[string]interface{}{
		"enabled": true,
		"label":   "synthetic value",
		"metadata": map[string]interface{}{
			"optional": nil,
		},
		"values": []interface{}{float64(3), "synthetic item", false, nil},
	}
	dynamic, err := customIndicatorDynamic(remote)
	var converted map[string]interface{}
	if err == nil {
		converted, err = customIndicatorJSONObject(context.Background(), dynamic)
	}

	got := struct {
		JSON    map[string]interface{}
		Matches bool
		Error   string
	}{JSON: converted, Matches: customIndicatorSchemaMatches(context.Background(), dynamic, remote)}
	if err != nil {
		got.Error = err.Error()
	}
	want := struct {
		JSON    map[string]interface{}
		Matches bool
		Error   string
	}{
		JSON: map[string]interface{}{
			"enabled": true,
			"label":   "synthetic value",
			"metadata": map[string]interface{}{
				"optional": nil,
			},
			"values": []interface{}{json.Number("3"), "synthetic item", false, nil},
		},
		Matches: true,
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("JSON round trip mismatch: got=%#v want=%#v", got, want)
	}
}
