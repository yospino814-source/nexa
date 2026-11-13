import type { AcceptInvitationResult, MyInvitation, Uuid } from "./types";

let mockMyInvitationsList: MyInvitation[] = [
  {
    id: "e4a2d8a0-1234-5678-90ab-cdef12345678",
    tenant_name: "Licorera La 15",
    branch_name: "Principal",
    role_code: "CAJERO",
    role_name: "Cajero",
    created_at: new Date().toISOString(),
  },
];

export async function mockMyInvitations(): Promise<MyInvitation[]> {
  return [...mockMyInvitationsList];
}

export async function mockAcceptInvitation(id: Uuid): Promise<AcceptInvitationResult> {
  const inv = mockMyInvitationsList.find((i) => i.id === id);
  mockMyInvitationsList = mockMyInvitationsList.filter((i) => i.id !== id);
  return {
    tenant_id: crypto.randomUUID(),
    branch_id: crypto.randomUUID(),
    role: inv?.role_code ?? "CAJERO",
  };
}

export async function mockRevokeInvitation(id: Uuid): Promise<void> {
  void id;
}
