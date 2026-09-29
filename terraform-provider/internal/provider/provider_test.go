package provider

import (
	"testing"

	"github.com/hashicorp/terraform-plugin-framework/types"
)

func TestConfiguredValue(t *testing.T) {
	cases := []struct {
		name          string
		value         types.String
		environment   string
		wantValue     string
		wantErrorText string
	}{
		{
			name:        "configured value takes precedence",
			value:       types.StringValue("https://synthetic.invalid"),
			environment: "https://environment.invalid",
			wantValue:   "https://synthetic.invalid",
		},
		{
			name:        "null value uses environment fallback",
			value:       types.StringNull(),
			environment: "https://environment.invalid",
			wantValue:   "https://environment.invalid",
		},
		{
			name:        "empty environment fallback remains empty",
			value:       types.StringNull(),
			environment: "",
			wantValue:   "",
		},
		{
			name:          "unknown value is rejected",
			value:         types.StringUnknown(),
			environment:   "https://environment.invalid",
			wantErrorText: "base_url must be known during provider configuration",
		},
	}

	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			t.Setenv("TRADER_BASE_URL", testCase.environment)
			value, err := configuredValue(testCase.value, "base_url", "TRADER_BASE_URL")
			errorText := ""
			if err != nil {
				errorText = err.Error()
			}
			got := struct {
				Value string
				Error string
			}{Value: value, Error: errorText}
			want := struct {
				Value string
				Error string
			}{Value: testCase.wantValue, Error: testCase.wantErrorText}
			if got != want {
				t.Fatalf("configured value mismatch: got=%#v want=%#v", got, want)
			}
		})
	}
}
