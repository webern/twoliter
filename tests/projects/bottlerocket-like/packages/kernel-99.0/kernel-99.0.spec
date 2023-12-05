%global _cross_first_party 1
%undefine _debugsource_packages

Name: %{_cross_os}kernel-99.0
Version: 99.0.0
Release: 0%{?dist}
Summary: Temporarily needed to satisfy rpm2kmodkits.
License: Apache-2.0 OR MIT

#Source0: kernel-devel-tar-xz

%description
%{summary}.

%package devel
Summary: Configured Linux kernel source for module building

%description devel
%{summary}.

%package archive
Summary: Archived Linux kernel source for module building

%description archive
%{summary}.

%package modules
Summary: Modules for the Linux kernel

%description modules
%{summary}.

%package headers
Summary: Header files for the Linux kernel for use by glibc

%description headers
%{summary}.

%prep
%setup -T -c
#%{SOURCE0} \
#%{_sourcedir}/kernel-devel.tar.xz

%build
mkdir bin

%install

# Create squashfs of kernel-devel files (ie. /usr/src/kernels/<version>).
#
# -no-exports:
# The filesystem does not need to be exported via NFS.
#
# -all-root:
# Make all files owned by root rather than the build user.
#
# -comp zstd:
# zstd offers compression ratios like xz and decompression speeds like lz4.
SQUASHFS_OPTS="-no-exports -all-root -comp zstd"
mkdir -p src_squashfs/%{version}
touch src_squashfs/%{version}/empty-file
touch kernel_devel_files
tar c -T kernel_devel_files | tar x -C src_squashfs/%{version}
mksquashfs src_squashfs kernel-devel.squashfs ${SQUASHFS_OPTS}

# Create a tarball of the same files, for use outside the running system.
# In theory we could extract these files with `unsquashfs`, but we do not want
# to require it to be installed on the build host, and it errors out when run
# inside Docker unless the limit for open files is lowered.
tar cf kernel-devel.tar src_squashfs/%{version} --transform='s|src_squashfs/%{version}|kernel-devel|'
xz -T0 kernel-devel.tar

install -D kernel-devel.squashfs %{buildroot}%{_cross_datadir}/bottlerocket/kernel-devel.squashfs
install -D kernel-devel.tar.xz %{buildroot}%{_cross_datadir}/bottlerocket/kernel-devel.tar.xz
install -d %{buildroot}%{kernel_sourcedir}

# This is because rpm2img expects it for the

%files

%files modules

%files headers

%files devel

%files archive
%{_cross_datadir}/bottlerocket/kernel-devel.tar.xz
%{_cross_datadir}/bottlerocket/kernel-devel.squashfs

%changelog
