package provider

import (
	"context"

	"github.com/hashicorp/terraform-plugin-framework/diag"
	"github.com/hashicorp/terraform-plugin-framework/path"
)

func customIndicatorPlanSchemas(ctx context.Context, plan customIndicatorModel) (map[string]interface{}, map[string]interface{}, diag.Diagnostics) {
	var diagnostics diag.Diagnostics
	inputSchema, err := jsonObjectFromDynamic(ctx, plan.InputSchema)
	if err != nil {
		diagnostics.AddAttributeError(path.Root("input_schema"), "Invalid input schema", err.Error())
	}
	outputSchema, err := jsonObjectFromDynamic(ctx, plan.OutputSchema)
	if err != nil {
		diagnostics.AddAttributeError(path.Root("output_schema"), "Invalid output schema", err.Error())
	}
	return inputSchema, outputSchema, diagnostics
}
