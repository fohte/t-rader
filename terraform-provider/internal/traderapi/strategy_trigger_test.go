package traderapi

import (
	"encoding/json"
	"github.com/fohte/t-rader/terraform-provider/internal/traderapi/gen"
	"reflect"
	"testing"

	"github.com/oapi-codegen/nullable"
)

func TestTriggerWithExactEventMatchPreservesIntegerPrecision(t *testing.T) {
	t.Parallel()

	type result struct {
		Trigger gen.Trigger
		Err     error
	}

	trigger, err := triggerWithExactEventMatch(gen.Trigger{}, []byte(`{"event_match":{"large_integer":9007199254740993}}`))
	got := result{Trigger: trigger, Err: err}
	want := result{
		Trigger: gen.Trigger{
			EventMatch: nullable.NewNullableWithValue(map[string]interface{}{
				"large_integer": json.Number("9007199254740993"),
			}),
		},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("triggerWithExactEventMatch output mismatch: got=%#v want=%#v", got, want)
	}
}
