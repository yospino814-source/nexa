import { ApiError } from "./client";
import type { CreateBusinessInput, OnboardingResult } from "./types";

export async function mockCreateBusiness(
  input: CreateBusinessInput,
): Promise<OnboardingResult> {
  const businessName = (input.business_name ?? "").trim();
  const branchName = (input.branch_name ?? "").trim();
  const fullName = (input.full_name ?? "").trim();

  // Validaciones en orden con longitud de caracteres
  const bnLen = Array.from(businessName).length;
  if (bnLen < 2 || bnLen > 80) {
    throw new ApiError(
      422,
      "Revisá los datos del negocio.",
      "ONBOARDING_INVALID",
      { field: "business_name" },
    );
  }

  const brLen = Array.from(branchName).length;
  if (brLen < 2 || brLen > 60) {
    throw new ApiError(
      422,
      "Revisá los datos del negocio.",
      "ONBOARDING_INVALID",
      { field: "branch_name" },
    );
  }

  const fnLen = Array.from(fullName).length;
  if (fnLen < 2 || fnLen > 80) {
    throw new ApiError(
      422,
      "Revisá los datos del negocio.",
      "ONBOARDING_INVALID",
      { field: "full_name" },
    );
  }

  return {
    tenant_id: crypto.randomUUID(),
    branch_id: crypto.randomUUID(),
  };
}

