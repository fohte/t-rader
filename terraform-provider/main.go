package main

import (
	"context"
	"log"

	"github.com/fohte/t-rader/terraform-provider/internal/provider"
	"github.com/hashicorp/terraform-plugin-framework/providerserver"
)

func main() {
	if err := providerserver.Serve(context.Background(), provider.New, providerserver.ServeOpts{
		Address: provider.ProviderAddress,
	}); err != nil {
		log.Fatal(err)
	}
}
