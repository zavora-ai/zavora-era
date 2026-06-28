import { useState, useCallback, useEffect } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import api, { getVendors, getFxRates, getSettings } from '../../api/client';
import type { Vendor } from '../../types';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import {
  Upload,
  FileImage,
  AlertTriangle,
  CheckCircle2,
  Loader2,
  ExternalLink,
} from 'lucide-react';

// === Types ===

interface OcrLineItem {
  description: string;
  quantity: number;
  unit_price: number;
  total: number;
  confidence: number;
}

interface OcrResult {
  vendor_name: string;
  vendor_name_confidence: number;
  date: string;
  date_confidence: number;
  total: number;
  total_confidence: number;
  vat_amount: number;
  vat_amount_confidence: number;
  currency?: string;
  currency_confidence?: number;
  line_items: OcrLineItem[];
  suggested_vendor_id?: string;
  suggested_vendor_name?: string;
}

interface CaptureResponse {
  capture_id: string;
  status: string;
  ocr_result: OcrResult;
}

interface ConfirmResponse {
  bill_id: string;
  bill_number: string;
}

type PageState = 'upload' | 'review' | 'confirmed';

const CONFIDENCE_THRESHOLD = 0.7;

// === Helper Components ===

function ConfidenceBadge({ confidence }: { confidence: number }) {
  if (confidence >= 0.9) {
    return (
      <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 rounded px-1.5 py-0.5">
        <CheckCircle2 className="w-3 h-3" />
        {Math.round(confidence * 100)}%
      </span>
    );
  }
  if (confidence >= CONFIDENCE_THRESHOLD) {
    return (
      <span className="inline-flex items-center gap-1 text-xs font-medium text-yellow-700 bg-yellow-50 rounded px-1.5 py-0.5">
        {Math.round(confidence * 100)}%
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 rounded px-1.5 py-0.5">
      <AlertTriangle className="w-3 h-3" />
      {Math.round(confidence * 100)}%
    </span>
  );
}

function FieldWrapper({
  label,
  confidence,
  children,
}: {
  label: string;
  confidence: number;
  children: React.ReactNode;
}) {
  const isLow = confidence < CONFIDENCE_THRESHOLD;
  return (
    <div>
      <label className="label flex items-center gap-2">
        {label}
        <ConfidenceBadge confidence={confidence} />
      </label>
      <div className={isLow ? 'ring-2 ring-red-300 rounded-md' : ''}>
        {children}
      </div>
      {isLow && (
        <p className="text-xs text-red-600 mt-1 flex items-center gap-1">
          <AlertTriangle className="w-3 h-3" /> Requires review — low confidence
        </p>
      )}
    </div>
  );
}

// === Main Page ===

export default function ReceiptCapturePage() {
  const [pageState, setPageState] = useState<PageState>('upload');
  const [captureId, setCaptureId] = useState<string>('');
  const [ocrResult, setOcrResult] = useState<OcrResult | null>(null);
  const [confirmedBill, setConfirmedBill] = useState<ConfirmResponse | null>(null);

  // Upload mutation
  const captureMutation = useMutation({
    mutationFn: (file: File) => {
      const formData = new FormData();
      formData.append('file', file);
      return api.post<CaptureResponse>('/receipts/capture', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
      });
    },
    onSuccess: (response) => {
      setCaptureId(response.data.capture_id);
      setOcrResult(response.data.ocr_result);
      setPageState('review');
    },
  });

  // Confirm mutation
  const confirmMutation = useMutation({
    mutationFn: (payload: {
      capture_id: string;
      vendor_id: string;
      currency?: string;
      fx_rate?: number;
      account_code?: string;
      adjustments: {
        vendor_name: string;
        date: string;
        total: number;
        vat_amount: number;
        line_items: OcrLineItem[];
      };
    }) => api.post<ConfirmResponse>('/receipts/confirm', payload),
    onSuccess: (response) => {
      setConfirmedBill(response.data);
      setPageState('confirmed');
    },
  });

  const handleFileUpload = useCallback(
    (file: File) => {
      captureMutation.mutate(file);
    },
    [captureMutation],
  );

  return (
    <div>
      <PageHeader
        title="Receipt Capture"
        subtitle="Upload a receipt or invoice image for OCR extraction"
      />

      {pageState === 'upload' && (
        <UploadZone
          onUpload={handleFileUpload}
          isLoading={captureMutation.isPending}
          error={captureMutation.error?.message}
        />
      )}

      {pageState === 'review' && ocrResult && (
        <ReviewPanel
          ocrResult={ocrResult}
          captureId={captureId}
          onConfirm={(payload) => confirmMutation.mutate(payload)}
          isSubmitting={confirmMutation.isPending}
          error={confirmMutation.error?.message}
          onReset={() => {
            setPageState('upload');
            setOcrResult(null);
            setCaptureId('');
          }}
        />
      )}

      {pageState === 'confirmed' && confirmedBill && (
        <SuccessPanel bill={confirmedBill} onReset={() => {
          setPageState('upload');
          setOcrResult(null);
          setCaptureId('');
          setConfirmedBill(null);
        }} />
      )}
    </div>
  );
}

// === Upload Zone ===

function UploadZone({
  onUpload,
  isLoading,
  error,
}: {
  onUpload: (file: File) => void;
  isLoading: boolean;
  error?: string;
}) {
  const [isDragging, setIsDragging] = useState(false);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragging(false);
      const file = e.dataTransfer.files[0];
      if (file && isValidFile(file)) {
        onUpload(file);
      }
    },
    [onUpload],
  );

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file && isValidFile(file)) {
        onUpload(file);
      }
    },
    [onUpload],
  );

  return (
    <div className="max-w-2xl mx-auto">
      <div
        className={`border-2 border-dashed rounded-xl p-12 text-center transition-colors ${
          isDragging
            ? 'border-blue-400 bg-blue-50'
            : 'border-gray-300 hover:border-gray-400'
        } ${isLoading ? 'pointer-events-none opacity-60' : ''}`}
        onDragOver={(e) => {
          e.preventDefault();
          setIsDragging(true);
        }}
        onDragLeave={() => setIsDragging(false)}
        onDrop={handleDrop}
      >
        {isLoading ? (
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="w-12 h-12 text-blue-500 animate-spin" />
            <p className="text-sm text-gray-600">
              Processing receipt with OCR...
            </p>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-4">
            <div className="w-16 h-16 rounded-full bg-gray-100 flex items-center justify-center">
              {isDragging ? (
                <FileImage className="w-8 h-8 text-blue-500" />
              ) : (
                <Upload className="w-8 h-8 text-gray-400" />
              )}
            </div>
            <div>
              <p className="text-base font-medium text-gray-700">
                Drag and drop a receipt here
              </p>
              <p className="text-sm text-gray-500 mt-1">
                or click to browse — supports JPEG, PNG, PDF
              </p>
            </div>
            <label className="btn-primary cursor-pointer">
              Choose File
              <input
                type="file"
                className="hidden"
                accept="image/jpeg,image/png,application/pdf"
                onChange={handleFileSelect}
              />
            </label>
          </div>
        )}
      </div>

      {error && (
        <div className="mt-4 p-3 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700">
          <strong>Upload failed:</strong> {error}
        </div>
      )}
    </div>
  );
}

function isValidFile(file: File): boolean {
  const validTypes = ['image/jpeg', 'image/png', 'application/pdf'];
  return validTypes.includes(file.type);
}

// === Review Panel ===

function ReviewPanel({
  ocrResult,
  captureId,
  onConfirm,
  isSubmitting,
  error,
  onReset,
}: {
  ocrResult: OcrResult;
  captureId: string;
  onConfirm: (payload: {
    capture_id: string;
    vendor_id: string;
    currency?: string;
    fx_rate?: number;
    adjustments: {
      vendor_name: string;
      date: string;
      total: number;
      vat_amount: number;
      line_items: OcrLineItem[];
    };
  }) => void;
  isSubmitting: boolean;
  error?: string;
  onReset: () => void;
}) {
  const [vendorName, setVendorName] = useState(ocrResult.vendor_name);
  const [selectedVendorId, setSelectedVendorId] = useState(
    ocrResult.suggested_vendor_id || '',
  );
  const [date, setDate] = useState(ocrResult.date);
  const [total, setTotal] = useState(ocrResult.total);
  const [vatAmount, setVatAmount] = useState(ocrResult.vat_amount);
  const [lineItems, setLineItems] = useState<OcrLineItem[]>(
    ocrResult.line_items,
  );

  // Multi-currency: capture the document currency + spot rate so a foreign
  // receipt (e.g. Amazon Ads in USD/EUR) posts at functional value instead of 1:1.
  const { data: settings } = useQuery<any>({ queryKey: ['settings'], queryFn: () => getSettings().then((r) => r.data) });
  const baseCurrency: string = settings?.base_currency ?? 'KES';
  const { data: fxRates = [] } = useQuery<any[]>({ queryKey: ['fx-rates'], queryFn: () => getFxRates().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  // Seed the currency from what the parser detected on the document, falling
  // back to base currency. The user can still change it.
  const detectedCurrency = (ocrResult.currency || '').toUpperCase();
  const [currency, setCurrency] = useState(detectedCurrency || baseCurrency);
  const [fxRate, setFxRate] = useState('1');
  const [fxTouched, setFxTouched] = useState(false);
  const lookupSpot = (ccy: string, d: string): number | null => {
    if (ccy === baseCurrency) return 1;
    const m = fxRates
      .filter((r) => r.from_ccy === ccy && r.to_ccy === baseCurrency && (!d || r.rate_date <= d))
      .sort((a, b) => (a.rate_date < b.rate_date ? 1 : -1));
    return m.length ? Number(m[0].rate) : null;
  };
  useEffect(() => {
    if (fxTouched) return;
    if (currency === baseCurrency) { setFxRate('1'); return; }
    const s = lookupSpot(currency, date);
    if (s != null) setFxRate(String(s));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currency, date, fxRates, baseCurrency]);

  const { data: vendors = [] } = useQuery<Vendor[]>({
    queryKey: ['vendors'],
    queryFn: () => getVendors().then((r) => r.data),
  });

  const hasLowConfidence =
    ocrResult.vendor_name_confidence < CONFIDENCE_THRESHOLD ||
    ocrResult.date_confidence < CONFIDENCE_THRESHOLD ||
    ocrResult.total_confidence < CONFIDENCE_THRESHOLD ||
    ocrResult.vat_amount_confidence < CONFIDENCE_THRESHOLD ||
    ocrResult.line_items.some((l) => l.confidence < CONFIDENCE_THRESHOLD);

  const updateLineItem = (
    index: number,
    field: keyof OcrLineItem,
    value: string | number,
  ) => {
    const updated = [...lineItems];
    updated[index] = { ...updated[index], [field]: value };
    setLineItems(updated);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onConfirm({
      capture_id: captureId,
      vendor_id: selectedVendorId,
      currency,
      fx_rate: parseFloat(fxRate) || 1,
      adjustments: {
        vendor_name: vendorName,
        date,
        total,
        vat_amount: vatAmount,
        line_items: lineItems,
      },
    });
  };

  return (
    <div className="max-w-4xl mx-auto">
      {hasLowConfidence && (
        <div className="mb-4 p-3 rounded-lg bg-yellow-50 border border-yellow-200 text-sm text-yellow-800 flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 flex-shrink-0" />
          Some fields have low confidence and require review before confirming.
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Header Fields */}
        <div className="bg-white rounded-lg border p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-900">
            Extracted Fields
          </h3>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* Vendor Name */}
            <FieldWrapper
              label="Vendor Name"
              confidence={ocrResult.vendor_name_confidence}
            >
              <input
                className="input"
                value={vendorName}
                onChange={(e) => setVendorName(e.target.value)}
              />
            </FieldWrapper>

            {/* Vendor Match */}
            <div>
              <label className="label">Match to Vendor</label>
              <select
                className="input"
                value={selectedVendorId}
                onChange={(e) => setSelectedVendorId(e.target.value)}
              >
                <option value="">— Select vendor —</option>
                {vendors.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name}
                  </option>
                ))}
              </select>
              {ocrResult.suggested_vendor_name && (
                <p className="text-xs text-gray-500 mt-1">
                  Suggested: <strong>{ocrResult.suggested_vendor_name}</strong>
                </p>
              )}
            </div>

            {/* Date */}
            <FieldWrapper label="Date" confidence={ocrResult.date_confidence}>
              <input
                type="date"
                className="input"
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </FieldWrapper>

            {/* Total */}
            <FieldWrapper
              label="Total"
              confidence={ocrResult.total_confidence}
            >
              <input
                type="number"
                step="0.01"
                className="input"
                value={total}
                onChange={(e) => setTotal(parseFloat(e.target.value) || 0)}
              />
            </FieldWrapper>

            {/* VAT Amount */}
            <FieldWrapper
              label="VAT Amount"
              confidence={ocrResult.vat_amount_confidence}
            >
              <input
                type="number"
                step="0.01"
                className="input"
                value={vatAmount}
                onChange={(e) => setVatAmount(parseFloat(e.target.value) || 0)}
              />
            </FieldWrapper>

            {/* Currency */}
            <div>
              <label className="label">Currency</label>
              <select
                className="input"
                value={currency}
                onChange={(e) => { setCurrency(e.target.value); setFxTouched(false); }}
              >
                {[baseCurrency, 'USD', 'EUR', 'GBP', 'KES', detectedCurrency]
                  .filter((c) => c)
                  .filter((c, i, a) => a.indexOf(c) === i)
                  .map((c) => (
                    <option key={c} value={c}>{c}</option>
                  ))}
              </select>
            </div>

            {/* FX Rate (only when foreign) */}
            {currency !== baseCurrency && (
              <div>
                <label className="label">
                  FX Rate (1 {currency} = ? {baseCurrency})
                </label>
                <input
                  type="number"
                  step="0.0001"
                  className="input"
                  value={fxRate}
                  onChange={(e) => { setFxRate(e.target.value); setFxTouched(true); }}
                />
                <p className="text-xs text-gray-500 mt-1">
                  Auto-filled from the spot rate on {date || 'the invoice date'}; edit if needed.
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Line Items */}
        <div className="bg-white rounded-lg border p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-900">Line Items</h3>

          {lineItems.length === 0 ? (
            <p className="text-sm text-gray-500">
              No line items extracted from receipt.
            </p>
          ) : (
            <div className="space-y-2">
              <div className="grid grid-cols-12 gap-2 text-xs font-medium text-gray-500 px-1">
                <span className="col-span-4">Description</span>
                <span className="col-span-2">Qty</span>
                <span className="col-span-2">Unit Price</span>
                <span className="col-span-2">Total</span>
                <span className="col-span-2">Confidence</span>
              </div>
              {lineItems.map((line, i) => (
                <div
                  key={i}
                  className={`grid grid-cols-12 gap-2 items-center ${
                    line.confidence < CONFIDENCE_THRESHOLD
                      ? 'bg-red-50 rounded p-1 ring-1 ring-red-200'
                      : ''
                  }`}
                >
                  <input
                    className="input col-span-4"
                    value={line.description}
                    onChange={(e) =>
                      updateLineItem(i, 'description', e.target.value)
                    }
                  />
                  <input
                    className="input col-span-2"
                    type="number"
                    value={line.quantity}
                    onChange={(e) =>
                      updateLineItem(i, 'quantity', +e.target.value)
                    }
                  />
                  <input
                    className="input col-span-2"
                    type="number"
                    step="0.01"
                    value={line.unit_price}
                    onChange={(e) =>
                      updateLineItem(i, 'unit_price', +e.target.value)
                    }
                  />
                  <input
                    className="input col-span-2"
                    type="number"
                    step="0.01"
                    value={line.total}
                    onChange={(e) =>
                      updateLineItem(i, 'total', +e.target.value)
                    }
                  />
                  <div className="col-span-2">
                    <ConfidenceBadge confidence={line.confidence} />
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Summary */}
        <div className="bg-gray-50 rounded-lg border p-4">
          <div className="flex justify-between items-center text-sm">
            <span className="text-gray-600">Subtotal (excl. VAT)</span>
            <span className="font-medium">
              {formatCurrency(total - vatAmount, currency)}
            </span>
          </div>
          <div className="flex justify-between items-center text-sm mt-1">
            <span className="text-gray-600">VAT</span>
            <span className="font-medium">{formatCurrency(vatAmount, currency)}</span>
          </div>
          <div className="flex justify-between items-center text-base font-bold mt-2 pt-2 border-t">
            <span>Total ({currency})</span>
            <span>{formatCurrency(total, currency)}</span>
          </div>
          {currency !== baseCurrency && (
            <div className="flex justify-between items-center text-sm mt-1 text-gray-500">
              <span>≈ in {baseCurrency} @ {fxRate}</span>
              <span>{formatCurrency(total * (parseFloat(fxRate) || 0), baseCurrency)}</span>
            </div>
          )}
        </div>

        {error && (
          <div className="p-3 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700">
            <strong>Confirmation failed:</strong> {error}
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-between items-center pt-2">
          <button
            type="button"
            onClick={onReset}
            className="btn-secondary"
          >
            Start Over
          </button>
          <button
            type="submit"
            className="btn-primary"
            disabled={isSubmitting || !selectedVendorId}
          >
            {isSubmitting ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" /> Confirming...
              </>
            ) : (
              <>
                <CheckCircle2 className="w-4 h-4" /> Confirm & Create Bill
              </>
            )}
          </button>
        </div>
      </form>
    </div>
  );
}

// === Success Panel ===

function SuccessPanel({
  bill,
  onReset,
}: {
  bill: ConfirmResponse;
  onReset: () => void;
}) {
  return (
    <div className="max-w-lg mx-auto text-center py-12">
      <div className="w-16 h-16 rounded-full bg-green-100 flex items-center justify-center mx-auto mb-4">
        <CheckCircle2 className="w-8 h-8 text-green-600" />
      </div>
      <h2 className="text-xl font-bold text-gray-900 mb-2">
        Bill Created Successfully
      </h2>
      <p className="text-sm text-gray-600 mb-6">
        Receipt has been processed and bill{' '}
        <strong>{bill.bill_number}</strong> has been created.
      </p>
      <div className="flex items-center justify-center gap-4">
        <Link
          to={`/bills`}
          className="btn-primary inline-flex items-center gap-2"
        >
          <ExternalLink className="w-4 h-4" /> View Bills
        </Link>
        <button onClick={onReset} className="btn-secondary">
          Capture Another
        </button>
      </div>
    </div>
  );
}
