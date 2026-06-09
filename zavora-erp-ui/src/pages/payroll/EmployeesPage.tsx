import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEmployees, createEmployeeApi } from '../../api/client';
import type { Employee } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Users, Shield } from 'lucide-react';

export default function EmployeesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: employees = [], isLoading } = useQuery<Employee[]>({
    queryKey: ['employees'],
    queryFn: () => getEmployees().then(r => r.data),
  });

  const columns: Column<Employee>[] = [
    { key: 'staff_number', header: 'Staff #', render: (r) => <span className="font-mono text-sm">{r.staff_number}</span> },
    {
      key: 'full_name', header: 'Employee',
      render: (r) => (
        <div>
          <p className="font-medium text-gray-900">{r.full_name}</p>
          <p className="text-xs text-gray-500">{r.employment_type}</p>
        </div>
      )
    },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => <span className="font-mono text-xs">{r.kra_pin}</span> },
    { key: 'basic_salary', header: 'Basic Salary', render: (r) => <span className="font-medium">{formatCurrency(r.basic_salary)}</span>, className: 'text-right' },
    { key: 'employment_type', header: 'Type', render: (r) => <span className="badge-info capitalize">{r.employment_type}</span> },
    {
      key: 'disability_exemption', header: 'Tax Relief',
      render: (r) => (
        <div className="text-xs">
          {r.disability_exemption && <span className="badge-warning mr-1">Disability</span>}
          {r.tax_relief > 0 && <span className="text-gray-500">KES {r.tax_relief.toLocaleString()}</span>}
        </div>
      )
    },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
    { key: 'start_date', header: 'Start Date', render: (r) => formatDate(r.start_date) },
  ];

  return (
    <div>
      <PageHeader
        title="Employees"
        subtitle={`${employees.length} employee${employees.length !== 1 ? 's' : ''} — Payroll register`}
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> Add Employee
          </button>
        }
      />
      <DataTable columns={columns} data={employees} loading={isLoading} emptyMessage="No employees yet. Add your first employee to start running payroll." />
      {showCreate && <CreateEmployeeModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateEmployeeModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();

  const [tab, setTab] = useState<'personal' | 'salary' | 'bank'>('personal');
  const [form, setForm] = useState({
    staff_number: '',
    full_name: '',
    kra_pin: '',
    nssf_number: '',
    nhif_number: '',
    helb_deduction: '',
    employment_type: 'Permanent',
    basic_salary: '',
    housing_allowance: '',
    transport_allowance: '',
    other_allowance: '',
    bank_name: '',
    bank_branch: '',
    account_number: '',
    tax_relief: '2400',
    disability_exemption: false,
    start_date: new Date().toISOString().split('T')[0],
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createEmployeeApi(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['employees'] });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      staff_number: form.staff_number,
      full_name: form.full_name,
      kra_pin: form.kra_pin,
      nssf_number: form.nssf_number || undefined,
      nhif_number: form.nhif_number || undefined,
      helb_deduction: form.helb_deduction ? parseFloat(form.helb_deduction) : undefined,
      employment_type: form.employment_type,
      basic_salary: parseFloat(form.basic_salary) || 0,
      allowances: {
        housing: parseFloat(form.housing_allowance) || 0,
        transport: parseFloat(form.transport_allowance) || 0,
        other: parseFloat(form.other_allowance) || 0,
      },
      bank_account: form.account_number ? {
        bank_name: form.bank_name,
        branch: form.bank_branch,
        account_number: form.account_number,
      } : undefined,
      tax_relief: parseFloat(form.tax_relief) || 2400,
      disability_exemption: form.disability_exemption,
      start_date: form.start_date,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Add Employee" subtitle="Kenya statutory payroll registration" size="lg">
      <form onSubmit={handleSubmit}>
        {/* Tabs */}
        <div className="flex gap-1 mb-6 border-b">
          {(['personal', 'salary', 'bank'] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors ${tab === t ? 'border-blue-600 text-blue-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}
            >
              {t === 'personal' ? 'Personal Details' : t === 'salary' ? 'Salary & Deductions' : 'Bank Details'}
            </button>
          ))}
        </div>

        {/* TAB: Personal Details */}
        {tab === 'personal' && (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">Staff Number *</label>
                <input className="input font-mono" value={form.staff_number} onChange={(e) => setForm({ ...form, staff_number: e.target.value })} placeholder="EMP-001" required />
              </div>
              <div>
                <label className="label">Employment Type *</label>
                <select className="input" value={form.employment_type} onChange={(e) => setForm({ ...form, employment_type: e.target.value })}>
                  <option value="Permanent">Permanent</option>
                  <option value="Contract">Contract</option>
                  <option value="Casual">Casual</option>
                </select>
              </div>
            </div>
            <div>
              <label className="label">Full Name *</label>
              <input className="input" value={form.full_name} onChange={(e) => setForm({ ...form, full_name: e.target.value })} placeholder="John Kamau Mwangi" required />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">KRA PIN *</label>
                <input className="input font-mono" value={form.kra_pin} onChange={(e) => setForm({ ...form, kra_pin: e.target.value.toUpperCase() })} placeholder="A00XXXXXXXX" maxLength={11} required />
                <p className="text-xs text-gray-400 mt-1">Required for PAYE filing</p>
              </div>
              <div>
                <label className="label">Start Date *</label>
                <input type="date" className="input" value={form.start_date} onChange={(e) => setForm({ ...form, start_date: e.target.value })} required />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">NSSF Number</label>
                <input className="input font-mono" value={form.nssf_number} onChange={(e) => setForm({ ...form, nssf_number: e.target.value })} placeholder="Optional" />
              </div>
              <div>
                <label className="label">NHIF Number</label>
                <input className="input font-mono" value={form.nhif_number} onChange={(e) => setForm({ ...form, nhif_number: e.target.value })} placeholder="Optional" />
              </div>
            </div>
          </div>
        )}

        {/* TAB: Salary & Deductions */}
        {tab === 'salary' && (
          <div className="space-y-4">
            <div>
              <label className="label">Basic Salary (KES/month) *</label>
              <input type="number" className="input" value={form.basic_salary} onChange={(e) => setForm({ ...form, basic_salary: e.target.value })} placeholder="50000" required />
            </div>

            <h4 className="text-sm font-medium text-gray-700 pt-2">Allowances</h4>
            <div className="grid grid-cols-3 gap-4">
              <div>
                <label className="label">Housing</label>
                <input type="number" className="input" value={form.housing_allowance} onChange={(e) => setForm({ ...form, housing_allowance: e.target.value })} placeholder="0" />
              </div>
              <div>
                <label className="label">Transport</label>
                <input type="number" className="input" value={form.transport_allowance} onChange={(e) => setForm({ ...form, transport_allowance: e.target.value })} placeholder="0" />
              </div>
              <div>
                <label className="label">Other</label>
                <input type="number" className="input" value={form.other_allowance} onChange={(e) => setForm({ ...form, other_allowance: e.target.value })} placeholder="0" />
              </div>
            </div>

            <hr className="my-4" />

            <h4 className="text-sm font-medium text-gray-700 flex items-center gap-2"><Shield className="w-4 h-4" /> Tax & Deductions</h4>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">HELB Monthly Deduction</label>
                <input type="number" className="input" value={form.helb_deduction} onChange={(e) => setForm({ ...form, helb_deduction: e.target.value })} placeholder="0" />
                <p className="text-xs text-gray-400 mt-1">Higher Education Loans Board</p>
              </div>
              <div>
                <label className="label">Personal Tax Relief (KES/month)</label>
                <input type="number" className="input" value={form.tax_relief} onChange={(e) => setForm({ ...form, tax_relief: e.target.value })} />
                <p className="text-xs text-gray-400 mt-1">Standard: KES 2,400/month</p>
              </div>
            </div>
            <div>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="checkbox" checked={form.disability_exemption} onChange={(e) => setForm({ ...form, disability_exemption: e.target.checked })} className="rounded" />
                <span className="text-sm">Disability exemption (tax exempt up to KES 150,000/month)</span>
              </label>
            </div>
          </div>
        )}

        {/* TAB: Bank Details */}
        {tab === 'bank' && (
          <div className="space-y-4">
            <p className="text-sm text-gray-500 mb-2">Bank details for salary payment via EFT/RTGS</p>
            <div>
              <label className="label">Bank Name</label>
              <input className="input" value={form.bank_name} onChange={(e) => setForm({ ...form, bank_name: e.target.value })} placeholder="e.g. Equity Bank" />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">Branch</label>
                <input className="input" value={form.bank_branch} onChange={(e) => setForm({ ...form, bank_branch: e.target.value })} placeholder="e.g. Westlands" />
              </div>
              <div>
                <label className="label">Account Number</label>
                <input className="input font-mono" value={form.account_number} onChange={(e) => setForm({ ...form, account_number: e.target.value })} placeholder="01XXXXXXXXXX" />
              </div>
            </div>
          </div>
        )}

        {/* Submit */}
        <div className="flex justify-between items-center pt-6 mt-6 border-t">
          <p className="text-xs text-gray-400">
            {form.full_name ? `Adding: ${form.full_name} (${form.staff_number || '...'})`  : 'Fill in employee details'}
          </p>
          <div className="flex gap-3">
            <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
            <button
              type="submit"
              className="btn-primary"
              disabled={mutation.isPending || !form.full_name || !form.kra_pin || !form.staff_number}
            >
              {mutation.isPending ? 'Saving...' : 'Save Employee'}
            </button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
