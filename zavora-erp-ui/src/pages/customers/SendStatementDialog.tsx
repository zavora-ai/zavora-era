import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { sendCustomerStatement } from '../../api/client';
import type { Customer } from '../../types';
import Modal from '../../components/shared/Modal';
import ProviderPreflight from '../../components/ProviderPreflight';
import { Mail, MessageCircle, Phone, AlertCircle, CheckCircle, Send } from 'lucide-react';

type Channel = 'email' | 'whatsapp' | 'sms';

export default function SendStatementDialog({ customer, onClose }: { customer: Customer; onClose: () => void }) {
  const email = customer.email?.find(e => e.is_primary)?.email || customer.email?.[0]?.email;
  const phone = customer.phone?.find(p => p.is_primary)?.number || customer.phone?.[0]?.number;

  const channels: { key: Channel; label: string; icon: typeof Mail; contact?: string }[] = [
    { key: 'email', label: 'Email', icon: Mail, contact: email },
    { key: 'whatsapp', label: 'WhatsApp', icon: MessageCircle, contact: phone },
    { key: 'sms', label: 'SMS', icon: Phone, contact: phone },
  ];

  const firstAvailable = channels.find(c => c.contact)?.key ?? 'email';
  const [channel, setChannel] = useState<Channel>(firstAvailable);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => sendCustomerStatement(customer.id, channel),
    onSuccess: () => { setDone(true); setTimeout(onClose, 1500); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Failed to queue statement.'),
  });

  const selected = channels.find(c => c.key === channel)!;

  return (
    <Modal open={true} onClose={onClose} title="Send Statement" subtitle={`Statement of account for ${customer.name}`}>
      <div className="space-y-5">
        {done ? (
          <div className="flex flex-col items-center text-center gap-2 py-6">
            <CheckCircle className="w-10 h-10 text-green-500" />
            <p className="text-sm font-medium text-gray-900">Statement queued for delivery</p>
            <p className="text-xs text-gray-500">It will be sent via {selected.label}.</p>
          </div>
        ) : (
          <>
            {error && (
              <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
                <AlertCircle className="w-4 h-4 shrink-0" /><span>{error}</span>
              </div>
            )}

            <div>
              <label className="label">Delivery channel</label>
              <div className="grid grid-cols-3 gap-2 mt-1">
                {channels.map(c => {
                  const disabled = !c.contact;
                  const active = channel === c.key;
                  return (
                    <button
                      key={c.key}
                      type="button"
                      disabled={disabled}
                      onClick={() => setChannel(c.key)}
                      className={`flex flex-col items-center gap-1 rounded-lg border p-3 text-sm transition-colors ${
                        disabled ? 'border-gray-100 text-gray-300 cursor-not-allowed'
                        : active ? 'border-indigo-400 bg-indigo-50 text-indigo-700'
                        : 'border-gray-200 text-gray-600 hover:border-gray-300'
                      }`}
                    >
                      <c.icon className="w-5 h-5" />
                      {c.label}
                    </button>
                  );
                })}
              </div>
            </div>

            <ProviderPreflight channel={channel} />

            <div className="bg-gray-50 rounded-lg p-3 text-sm">
              {selected.contact ? (
                <p className="text-gray-700">Will be sent to <span className="font-medium">{selected.contact}</span></p>
              ) : (
                <p className="text-amber-700 flex items-center gap-1.5">
                  <AlertCircle className="w-4 h-4" />
                  No {channel === 'email' ? 'email' : 'phone'} on file. Add one on the customer record first.
                </p>
              )}
            </div>

            <div className="flex justify-end gap-3 pt-2 border-t">
              <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
              <button
                type="button"
                onClick={() => { setError(null); mutation.mutate(); }}
                className="btn-primary"
                disabled={mutation.isPending || !selected.contact}
              >
                <Send className="w-4 h-4" /> {mutation.isPending ? 'Sending...' : 'Send Statement'}
              </button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
