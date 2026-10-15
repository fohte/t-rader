package provider

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"reflect"
	"strconv"

	"github.com/hashicorp/terraform-plugin-framework/attr"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/hashicorp/terraform-plugin-go/tftypes"
)

func jsonObjectFromDynamic(ctx context.Context, value types.Dynamic) (map[string]interface{}, error) {
	if value.IsNull() || value.IsUnknown() {
		return nil, errors.New("must be a known JSON object")
	}
	underlying := value.UnderlyingValue()
	if underlying == nil || underlying.IsNull() || underlying.IsUnknown() {
		return nil, errors.New("must be a known JSON object")
	}
	terraformValue, err := underlying.ToTerraformValue(ctx)
	if err != nil {
		return nil, fmt.Errorf("convert Terraform value: %w", err)
	}
	if !terraformValue.IsFullyKnown() {
		return nil, errors.New("must not contain unknown values")
	}
	valueAsJSON, err := terraformValueToJSON(terraformValue)
	if err != nil {
		return nil, err
	}
	object, ok := valueAsJSON.(map[string]interface{})
	if !ok {
		return nil, errors.New("must be a JSON object")
	}
	return object, nil
}

func jsonObjectMatchesPrior(ctx context.Context, prior types.Dynamic, remote interface{}) bool {
	// API の JSON 数値は float64 になるため、同じ値なら Terraform 側の dynamic 型を維持します。
	priorObject, err := jsonObjectFromDynamic(ctx, prior)
	if err != nil {
		return false
	}
	return jsonValueEqual(priorObject, remote)
}

func jsonValueEqual(left, right interface{}) bool {
	if leftMap, ok := left.(map[string]interface{}); ok {
		rightMap, ok := right.(map[string]interface{})
		if !ok || len(leftMap) != len(rightMap) {
			return false
		}
		for key, leftValue := range leftMap {
			rightValue, exists := rightMap[key]
			if !exists || !jsonValueEqual(leftValue, rightValue) {
				return false
			}
		}
		return true
	}
	if leftValues, ok := left.([]interface{}); ok {
		rightValues, ok := right.([]interface{})
		if !ok || len(leftValues) != len(rightValues) {
			return false
		}
		for index, leftValue := range leftValues {
			if !jsonValueEqual(leftValue, rightValues[index]) {
				return false
			}
		}
		return true
	}
	if leftNumber, ok := jsonNumber(left); ok {
		rightNumber, ok := jsonNumber(right)
		return ok && leftNumber == rightNumber
	}
	return reflect.DeepEqual(left, right)
}

func jsonNumber(value interface{}) (float64, bool) {
	switch value := value.(type) {
	case json.Number:
		parsed, err := strconv.ParseFloat(value.String(), 64)
		return parsed, err == nil
	case float64:
		return value, true
	default:
		return 0, false
	}
}

func terraformValueToJSON(value tftypes.Value) (interface{}, error) {
	if value.IsNull() {
		return nil, nil
	}
	switch {
	case value.Type().Is(tftypes.String):
		var result string
		if err := value.As(&result); err != nil {
			return nil, err
		}
		return result, nil
	case value.Type().Is(tftypes.Number):
		var result *big.Float
		if err := value.As(&result); err != nil {
			return nil, err
		}
		return json.Number(result.Text('g', -1)), nil
	case value.Type().Is(tftypes.Bool):
		var result bool
		if err := value.As(&result); err != nil {
			return nil, err
		}
		return result, nil
	case value.Type().Is(tftypes.Object{}) || value.Type().Is(tftypes.Map{}):
		var values map[string]tftypes.Value
		if err := value.As(&values); err != nil {
			return nil, err
		}
		result := make(map[string]interface{}, len(values))
		for name, child := range values {
			converted, err := terraformValueToJSON(child)
			if err != nil {
				return nil, fmt.Errorf("convert object attribute %q: %w", name, err)
			}
			result[name] = converted
		}
		return result, nil
	case value.Type().Is(tftypes.List{}) || value.Type().Is(tftypes.Set{}) || value.Type().Is(tftypes.Tuple{}):
		var values []tftypes.Value
		if err := value.As(&values); err != nil {
			return nil, err
		}
		result := make([]interface{}, len(values))
		for index, child := range values {
			converted, err := terraformValueToJSON(child)
			if err != nil {
				return nil, fmt.Errorf("convert array element %d: %w", index, err)
			}
			result[index] = converted
		}
		return result, nil
	default:
		return nil, fmt.Errorf("unsupported Terraform type %s", value.Type())
	}
}

func dynamicFromJSON(value interface{}) (types.Dynamic, error) {
	converted, err := jsonValueToAttribute(value)
	if err != nil {
		return types.DynamicNull(), err
	}
	return types.DynamicValue(converted), nil
}

func jsonValueToAttribute(value interface{}) (attr.Value, error) {
	switch value := value.(type) {
	case nil:
		return types.DynamicNull(), nil
	case string:
		return types.StringValue(value), nil
	case bool:
		return types.BoolValue(value), nil
	case float64:
		return types.NumberValue(big.NewFloat(value)), nil
	case map[string]interface{}:
		attributeTypes := make(map[string]attr.Type, len(value))
		attributes := make(map[string]attr.Value, len(value))
		for name, child := range value {
			converted, err := jsonValueToAttribute(child)
			if err != nil {
				return nil, fmt.Errorf("convert object attribute %q: %w", name, err)
			}
			attributes[name] = converted
			attributeTypes[name] = converted.Type(context.Background())
		}
		object, diagnostics := types.ObjectValue(attributeTypes, attributes)
		if diagnostics.HasError() {
			return nil, fmt.Errorf("create object value: %v", diagnostics)
		}
		return object, nil
	case []interface{}:
		elementTypes := make([]attr.Type, len(value))
		elements := make([]attr.Value, len(value))
		for index, child := range value {
			converted, err := jsonValueToAttribute(child)
			if err != nil {
				return nil, fmt.Errorf("convert array element %d: %w", index, err)
			}
			elements[index] = converted
			elementTypes[index] = converted.Type(context.Background())
		}
		tuple, diagnostics := types.TupleValue(elementTypes, elements)
		if diagnostics.HasError() {
			return nil, fmt.Errorf("create tuple value: %v", diagnostics)
		}
		return tuple, nil
	default:
		return nil, fmt.Errorf("unsupported JSON value %T", value)
	}
}
