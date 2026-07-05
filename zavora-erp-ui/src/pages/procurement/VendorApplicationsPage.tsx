import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getVendorApplications, approveVendorApplication, rejectVendorApplication,
} from '../../api/client';
import { formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_APPROVE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import { CheckCircle, XCircle } from 'lucide-react';

interface VendorApplication {
  id: string;
  email: string;
  display_name: string;
  company_name: string;
  kra_pin?: string;
  phone?: string;
  status: string;
  vendor_id?: string;
  created_at: string;
}

export default function VendorApplicationsPage() {
  const queryClient = useQueryClient();
  const canApprove = hasRole(ROLES_APPROVE);

  const { data: apps = [], isLoading } = useQuery<VendorApplication[]>({
    queryKey: ['vendor-applications'],
    queryFn: () => getVendorApplications().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['vendor-applications'] });
  const approveMut = useMutation({ mutationFn: (id: string) => approveVendorApplication(id), onSuccess: invalidate });
  const rejectMut = useMutation({ mutationFn: (id: string) => rejectVendorApplication(id), onSuccess: invalidate });

  const columns: Column<VendorApplication>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'company_name', header: 'Company', render: (r) => <span className="font-medium text-gray-900">{r.company_name}</span> },
    { key: 'display_name', header: 'Contact', render: (r) => <div><p>{r.display_name}</p><p className="text-xs text-gray-400">{r.email}</p></div> },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => r.kra_pin || '—' },
    { key: 'phone', header: 'Phone', render: (r) => r.phone || '—' },
    { key: 'created_at', header: 'Applied', render: (r) => formatDate(r.created_at) },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {r.status === 'pending' && canApprove && (
            <>
              <button
                onClick={() => approveMut.mutate(r.id)}
                disabled={approveMut.isPending}
                className="btn-success text-xs py-1 px-2"
                title="Approve & create the vendor master"
              >
                <CheckCircle className="w-3 h-3" /> Approve
              </button>
              <button
                onClick={() => { if (confirm(`Reject ${r.company_name}'s application?`)) rejectMut.mutate(r.id); }}
                disabled={rejectMut.isPending}
                className="btn-secondary text-xs py-1 px-2 text-red-600"
                title="Reject application"
              >
                <XCircle className="w-3 h-3" /> Reject
              </button>
            </>
          )}
          {r.status === 'active' && <span className="text-xs text-emerald-600">Approved</span>}
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Vendor Applications"
        subtitle="Suppliers who registered on the vendor portal. Approve to create their vendor master and grant portal access."
      />
      <DataTable
        columns={columns}
        data={apps}
        loading={isLoading}
        emptyMessage="No vendor applications yet. Suppliers self-register from the vendor portal."
      />
    </div>
  );
}
