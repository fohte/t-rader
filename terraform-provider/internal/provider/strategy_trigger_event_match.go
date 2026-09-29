package provider

import (
	"context"
	"encoding/json"
	"fmt"
	"math/big"

	"github.com/hashicorp/terraform-plugin-framework/attr"
	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/oapi-codegen/nullable"
)

func eventMatchForCreate(ctx context.Context, value types.Dynamic) (interface{}, error) {
	if value.IsNull() || value.IsUnknown() {
		return nil, nil
	}
	return dynamicEventMatchToJSON(ctx, value)
}

func eventMatchForUpdate(ctx context.Context, value types.Dynamic) (interface{}, error) {
	if value.IsUnknown() {
		return nil, nil
	}
	if value.IsNull() {
		return json.RawMessage("null"), nil
	}
	return dynamicEventMatchToJSON(ctx, value)
}

func dynamicEventMatchToJSON(ctx context.Context, value types.Dynamic) (map[string]interface{}, error) {
	converted, err := attrValueToJSON(ctx, value.UnderlyingValue())
	if err != nil {
		return nil, err
	}
	object, ok := converted.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("event_match must be an object, got %T", converted)
	}
	return object, nil
}

func attrValueToJSON(ctx context.Context, value attr.Value) (interface{}, error) {
	if value == nil || value.IsNull() {
		return nil, nil
	}
	if value.IsUnknown() {
		return nil, fmt.Errorf("event_match cannot contain unknown values")
	}
	switch typedValue := value.(type) {
	case types.Bool:
		return typedValue.ValueBool(), nil
	case types.Float32:
		return typedValue.ValueFloat32(), nil
	case types.Float64:
		return typedValue.ValueFloat64(), nil
	case types.Int32:
		return typedValue.ValueInt32(), nil
	case types.Int64:
		return typedValue.ValueInt64(), nil
	case types.Number:
		return json.Number(typedValue.ValueBigFloat().Text('g', -1)), nil
	case types.String:
		return typedValue.ValueString(), nil
	case types.Dynamic:
		return attrValueToJSON(ctx, typedValue.UnderlyingValue())
	case types.List:
		return attrValuesToJSON(ctx, typedValue.Elements())
	case types.Set:
		return attrValuesToJSON(ctx, typedValue.Elements())
	case types.Tuple:
		return attrValuesToJSON(ctx, typedValue.Elements())
	case types.Map:
		return attrMapToJSON(ctx, typedValue.Elements())
	case types.Object:
		return attrMapToJSON(ctx, typedValue.Attributes())
	default:
		return nil, fmt.Errorf("unsupported event_match value type %T", value)
	}
}

func attrValuesToJSON(ctx context.Context, values []attr.Value) ([]interface{}, error) {
	converted := make([]interface{}, 0, len(values))
	for _, value := range values {
		item, err := attrValueToJSON(ctx, value)
		if err != nil {
			return nil, err
		}
		converted = append(converted, item)
	}
	return converted, nil
}

func attrMapToJSON(ctx context.Context, values map[string]attr.Value) (map[string]interface{}, error) {
	converted := make(map[string]interface{}, len(values))
	for key, value := range values {
		item, err := attrValueToJSON(ctx, value)
		if err != nil {
			return nil, fmt.Errorf("event_match.%s: %w", key, err)
		}
		converted[key] = item
	}
	return converted, nil
}

func dynamicEventMatchFromAPI(ctx context.Context, value nullable.Nullable[map[string]interface{}]) (types.Dynamic, error) {
	if !value.IsSpecified() || value.IsNull() {
		return types.DynamicNull(), nil
	}
	converted, err := jsonValueToAttr(ctx, value.GetOrEmpty())
	if err != nil {
		return types.DynamicNull(), err
	}
	return types.DynamicValue(converted), nil
}

func jsonValueToAttr(ctx context.Context, value interface{}) (attr.Value, error) {
	switch typedValue := value.(type) {
	case nil:
		return types.DynamicNull(), nil
	case bool:
		return types.BoolValue(typedValue), nil
	case float64:
		return types.NumberValue(new(big.Float).SetFloat64(typedValue)), nil
	case json.Number:
		number, _, err := big.ParseFloat(string(typedValue), 10, 256, big.ToNearestEven)
		if err != nil {
			return nil, fmt.Errorf("parse event_match number: %w", err)
		}
		return types.NumberValue(number), nil
	case int:
		return types.NumberValue(new(big.Float).SetInt64(int64(typedValue))), nil
	case int32:
		return types.NumberValue(new(big.Float).SetInt64(int64(typedValue))), nil
	case int64:
		return types.NumberValue(new(big.Float).SetInt64(typedValue)), nil
	case string:
		return types.StringValue(typedValue), nil
	case []interface{}:
		elements := make([]attr.Value, 0, len(typedValue))
		elementTypes := make([]attr.Type, 0, len(typedValue))
		for _, item := range typedValue {
			converted, err := jsonValueToAttr(ctx, item)
			if err != nil {
				return nil, err
			}
			elements = append(elements, converted)
			elementTypes = append(elementTypes, converted.Type(ctx))
		}
		tuple, diagnostics := types.TupleValue(elementTypes, elements)
		if diagnostics.HasError() {
			return nil, diagnosticsError(diagnostics)
		}
		return tuple, nil
	case map[string]interface{}:
		attributes := make(map[string]attr.Value, len(typedValue))
		attributeTypes := make(map[string]attr.Type, len(typedValue))
		for key, item := range typedValue {
			converted, err := jsonValueToAttr(ctx, item)
			if err != nil {
				return nil, fmt.Errorf("event_match.%s: %w", key, err)
			}
			attributes[key] = converted
			attributeTypes[key] = converted.Type(ctx)
		}
		object, diagnostics := types.ObjectValue(attributeTypes, attributes)
		if diagnostics.HasError() {
			return nil, diagnosticsError(diagnostics)
		}
		return object, nil
	default:
		return nil, fmt.Errorf("unsupported event_match API value type %T", value)
	}
}

func diagnosticsError(diagnostics diag.Diagnostics) error {
	for _, diagnostic := range diagnostics {
		if diagnostic.Severity() == diag.SeverityError {
			return fmt.Errorf("%s: %s", diagnostic.Summary(), diagnostic.Detail())
		}
	}
	return fmt.Errorf("Terraform value conversion failed")
}
