import { useEffect, useMemo, useState } from 'preact/hooks';
import {
  Badge,
  Banner,
  Button,
  Dialog,
  Icon,
  IconButton,
  Table,
  TextField,
  type TableColumn,
} from 'rimstudio-ui';
import {
  devFsHome,
  devFsList,
  normalizeError,
  type ApiError,
  type DevFsEntry,
  type DevFsHome,
  type DevFsListing,
} from '~/shared/ipc';
import { t } from '~/shared/i18n';
import { pickerRequest, type PickerRequest } from './dialogs';

function joinPath(base: string, name: string): string {
  const sep = base.includes('\\') && !base.includes('/') ? '\\' : '/';
  return base.endsWith(sep) ? `${base}${name}` : `${base}${sep}${name}`;
}

/** The dialog behind pickFolder and pickFile in browser mode; built on /dev/fs/list and /dev/fs/home. */
function PickerDialog({ request }: { request: PickerRequest }) {
  const fileMode = request.mode === 'file';
  const [home, setHome] = useState<DevFsHome>();
  const [path, setPath] = useState<string | undefined>(request.start);
  const [pathText, setPathText] = useState(request.start ?? '');
  const [listing, setListing] = useState<DevFsListing>();
  const [error, setError] = useState<ApiError>();
  const [selected, setSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    let live = true;
    devFsHome().then(
      (h) => {
        if (!live) return;
        setHome(h);
        setPath((p) => p ?? h.home);
      },
      (e: unknown) => live && setError(normalizeError(e)),
    );
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!path) return undefined;
    let live = true;
    setError(undefined);
    devFsList(path, fileMode).then(
      (l) => {
        if (!live) return;
        setListing(l);
        setPathText(l.path);
        setSelected(new Set());
      },
      (e: unknown) => live && setError(normalizeError(e)),
    );
    return () => {
      live = false;
    };
  }, [path, fileMode]);

  const finish = (value: string | null): void => {
    pickerRequest.value = null;
    request.resolve(value);
  };

  const columns = useMemo<TableColumn<DevFsEntry>[]>(
    () => [
      {
        key: 'name',
        header: t('picker.name'),
        render: (e) => (
          <span class="inline-flex items-center gap-2">
            <Icon name={e.kind === 'dir' ? 'folder' : 'file'} />
            <span>{e.name}</span>
            {e.isModFolder ? <Badge tone="success">{t('picker.mod')}</Badge> : null}
          </span>
        ),
      },
      {
        key: 'kind',
        header: t('picker.kind'),
        render: (e) => (e.kind === 'dir' ? t('picker.kind.dir') : t('picker.kind.file')),
        widthClass: 'w-28',
      },
    ],
    [],
  );

  const chosenFile = fileMode ? [...selected][0] : undefined;
  const canChoose = fileMode ? Boolean(chosenFile) : Boolean(listing);

  const activate = (entry: DevFsEntry): void => {
    if (!listing) return;
    const full = joinPath(listing.path, entry.name);
    if (entry.kind === 'dir') setPath(full);
    else if (fileMode) finish(full);
  };

  return (
    <Dialog
      open
      size="lg"
      title={fileMode ? t('picker.title.file') : t('picker.title')}
      closeLabel={t('picker.close')}
      onClose={() => finish(null)}
      footer={
        <>
          <Button onClick={() => finish(null)}>{t('picker.cancel')}</Button>
          <Button
            variant="primary"
            disabled={!canChoose}
            onClick={() =>
              finish(
                fileMode
                  ? joinPath(listing?.path ?? '', chosenFile ?? '')
                  : (listing?.path ?? null),
              )
            }
          >
            {fileMode ? t('picker.choose.file') : t('picker.choose')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        <div class="flex items-center gap-2">
          <IconButton
            icon="arrow-up"
            label={t('picker.up')}
            disabled={!listing?.parent}
            onClick={() => listing?.parent && setPath(listing.parent)}
          />
          <div class="flex-1">
            <TextField
              aria-label={t('picker.path')}
              value={pathText}
              onValueChange={setPathText}
              onKeyDown={(e) => {
                if (e.key === 'Enter') setPath(pathText);
              }}
            />
          </div>
        </div>
        {home && home.places.length > 0 ? (
          <div role="group" aria-label={t('picker.places')} class="flex flex-wrap gap-2">
            {home.places.map((place) => (
              <Button key={place.path} size="sm" icon="folder" onClick={() => setPath(place.path)}>
                {place.label}
              </Button>
            ))}
          </div>
        ) : null}
        {error ? (
          <Banner tone="error" title={t('picker.error')}>
            {error.message}
          </Banner>
        ) : null}
        <div class="max-h-80 overflow-auto border border-line">
          <Table
            label={t('picker.title')}
            columns={columns}
            rows={listing?.entries ?? []}
            getKey={(e) => e.name}
            selection={fileMode ? 'single' : 'none'}
            selectedKeys={selected}
            onSelectionChange={setSelected}
            onRowActivate={activate}
            dense
            emptyText={t('picker.empty')}
          />
        </div>
      </div>
    </Dialog>
  );
}

/** Mount once in the shell; renders the picker when pickFolder or pickFile was called. */
export function FolderPickerHost() {
  const request = pickerRequest.value;
  return request ? <PickerDialog key={request.id} request={request} /> : null;
}
